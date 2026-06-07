---
id: "0014"
title: Persisted Sync Ledger and Honest Job Outcomes
status: active
tags: [architecture, sync, ledger, ui]
created: 2026-06-06
---

# ADR-0014: Persisted Sync Ledger and Honest Job Outcomes

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The sync layer gains a durable, per-`(subject, kind)` **ledger** and a structured per-job **Outcome** contract. A job no
longer reports success merely by returning `Ok(())`; it reports *what it accomplished* — `Synced` (with a row count),
`Empty`, `Blocked`, `NotReady`, `Failed`, or `Skipped` — and the engine records that outcome, the rows touched, the
attempt/success timestamps, and the next-eligible time to a persisted `sync_ledger` table. The ledger is the single home
for "what data is actually present and when each job is next due," and it survives restarts.

This is an evolution of [ADR-0002](0002-sync-render-separation.md), not a departure from it: the ledger lives entirely
on the **sync** side, the engine still owns all execution and scheduling, and the UI still only **observes** (now with
enough signal to distinguish synced-with-data from empty / blocked / not-ready / failed, rather than showing green over
nothing).

## Context

Under ADR-0002 a job's only signal was `Result<(), Error>`. `Ok(())` flipped the sync chip green whether the job wrote a
thousand rows, wrote zero rows, or skipped its work because a precondition was missing. Three problems share that one
root cause — the layer equates "ran without erroring" with "the data is present and fresh":

1. **Outcome blindness.** `Phase` (`Syncing | Done | Failed | BackingOff`) cannot represent "synced but empty/blocked."
   The abyssals job, for example, returns `Ok(())` with zero results in single-digit milliseconds (no ESI call) while
   the chip reports a clean sync.
2. **No cross-restart freshness.** The scheduler's `next_run_at` lived only in the in-memory `Schedule`. On startup
   everything was marked due-now, so a restart re-fetched everything from ESI even if it had synced seconds earlier —
   wasted calls and rate-limit exposure.
3. **No durable record of work done.** Nothing on disk distinguished a subject that owns nothing from one whose data was
   never fetched, so neither the UI nor the engine could reason about freshness across runs.

The fix needs a memory of work done that outlives the process and a vocabulary richer than a bare `Result`.

## Decision

### A structured `Outcome`

Sync jobs express what they accomplished through `sync::Outcome` (`src/sync/outcome.rs`):

- `Synced { rows_touched }` — fetched/derived real data; `rows_touched` records how much.
- `Empty` — ran successfully and there is legitimately nothing (you own none of this).
- `Blocked { reason }` — a precondition the user controls is missing (e.g. an ungranted scope).
- `NotReady` — an upstream dependency has not been synced yet; the job should run after its inputs.
- `Failed { reason }` — the attempt errored.
- `Skipped { reason }` — deliberately not run this cycle (e.g. a disabled feature).

`Outcome` exposes exactly the three projections the ledger persists: `label()` (the stored token), `rows_touched()`, and
`reason()`. Mapping a job result to an outcome, and deciding which outcomes count as a freshness-bearing *success*, is
the engine's job and is layered in by later work; this record establishes the type and the storage it writes to.

### The `sync_ledger` table

A single table, one row per `(subject_type, subject_id, kind)` — those three columns together form the primary key:

- `subject_type` — `TEXT`, CHECK in (`character`, `corporation`).
- `subject_id` — `INTEGER`.
- `kind` — `TEXT`; the `JobKind`.
- `outcome` — `TEXT`, CHECK in the six outcome tokens; the last recorded outcome.
- `rows_touched` — `INTEGER NOT NULL DEFAULT 0`; rows from the last attempt.
- `last_reason` — `TEXT`; the explanation for a blocked/failed/skipped outcome.
- `last_attempt_at` — `TEXT NOT NULL`; rfc3339, bumped on every attempt.
- `last_success_at` — `TEXT`; rfc3339, preserved across non-success attempts.
- `next_eligible_at` — `TEXT`; rfc3339, when this job is next due.

The repo (`src/store/repo/sync_ledger.rs`) exposes an `upsert` keyed on the primary key plus reads (`all`,
`for_subject`, `get`). The upsert preserves a previously recorded `last_success_at` when the current attempt is not a
success (`COALESCE(excluded.last_success_at, sync_ledger.last_success_at)`), so a later empty/blocked/failed attempt
never erases proven freshness. Timestamps are stored as rfc3339 `TEXT`, matching the outbox convention.

### Migration renumber

The ledger is migration **0001**. Every pre-existing migration (`0001`–`0060`) is renamed `+1` to `0002`–`0061` with
its SQL byte-identical; only the numeric prefix changes. This deliberately changes migration checksums and trips
`migration_guard` on existing installs — which is acceptable for an unpublished binary that edits migrations in place,
and the guard backs up the legacy `pod.db` rather than deleting it.

### Stays within ADR-0002

The ledger is sync-owned state. The engine reads it to schedule and writes it as jobs complete; the render layer never
writes to it and the control channel still carries no data. The UI's only new capability is *observation* — it can
distinguish honest outcomes — which is squarely within "the UI only observes."

## Affected Areas

- `src/sync/outcome.rs` — the `Outcome` type; re-exported from `src/sync.rs`.
- `migrations/0001_create_sync_ledger.sql` — the new table; `migrations/0002_*`–`0061_*` — the renamed prior set.
- `src/store/model/sync_ledger.rs` — the row model; re-exported as `SyncLedger`.
- `src/store/repo/sync_ledger.rs` — `upsert` + `all` / `for_subject` / `get`.
- `src/store/migration_guard.rs` — unchanged, but the renumber relies on its legacy-backup behavior.

Later tasks in this effort (not part of this record) wire the engine to record outcomes, hydrate the schedule from the
ledger on startup, chain `AssetSync → CharacterAbyssals`, and surface honest state in `SyncStatus`/`Phase` and the sync
chip.

## Consequences

### Positive

- "Synced" can mean "data is present"; empty / blocked / not-ready / failed each have a durable, explainable record.
- Freshness survives restarts, so the engine can resume scheduling instead of re-syncing everything — rate-limit safe.
- A single per-`(subject, kind)` home for outcome + freshness replaces scattered in-memory phase state.

### Negative

- The renumber changes every migration checksum, so existing installs are backed up and re-migrated on next launch.
- The ledger adds a write per job attempt; jobs and the engine must agree on which outcomes count as a success for
  `last_success_at` (resolved by later work).
- `Empty` vs `Blocked` is only as honest as each job's self-report; auditing every job to report truthfully is follow-up
  work in this same effort.

## References

- ADR-0002 — Sync/Render Separation and Aggregation Chaining (`0002-sync-render-separation.md`). The governing
  constraint this record evolves: sync owns execution/scheduling, the UI only observes.
- ADR-0010 — ESI Write Path / Durable Outbox (`0010-esi-write-path-outbox.md`). The ledger mirrors the outbox's repo and
  rfc3339-`TEXT` conventions.
- Spec — "Trustworthy Sync — Persisted Ledger, Honest Outcomes & Cross-Restart Freshness" (gest artifact `kwnmuvmn`).
