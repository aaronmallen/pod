---
id: "0036"
title: Freshness-First Sync Status via Engine-Emitted Seed Events
status: active
tags: [architecture, sync, ui, ledger]
created: 2026-06-23
---

# ADR-0036: Freshness-First Sync Status via Engine-Emitted Seed Events

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The sync chip and popover are reworked from a finite progress fraction (X/70) to a freshness model: at rest the UI reads
"up to date," it gets loud only for genuinely stale/failed/blocked/needs-reauth work or a real initial catch-up, and
routine background refreshes never change the headline. The enabling mechanism is a new `Event::Seeded` emitted by the
engine at launch — reusing the ledger rows it already loads in `seeds_for` — so the in-memory `SyncStatus` reflects
persisted freshness immediately instead of starting blind every process. The UI remains a pure observer (no new UI to DB
read path), and the `Phase` enum is left unchanged; the fresh / catching-up / attention vocabulary is computed by a
single shared derivation function that both the chip and popover consume.

This is an evolution of ADR-0014, which built the `sync_ledger` explicitly to feed this UX and named "surface honest
state in the chip" as deferred follow-up. It honors ADR-0002: the engine owns the ledger and emits events; the UI only
observes.

## Context

The sync status surface never reads as "complete." The popover header shows an X/70 fraction that oscillates around
58-60/70 and never reaches 70/70, and rows like "Clones / Profile / Wallet — Queued" sit with nothing apparently
happening. Investigation (a one-hour log capture plus the code) found this is not a stuck sync — the engine behaves
correctly — but a flawed status model with three causes:

1. Empty is excluded from the count. `build_model` (`src/ui/components/sync_popover.rs`) counts only `RowState::Done`;
   `job_stats` counts `Done | Empty`; the dead-code `SyncStatus::done()` counts both. The chip and popover disagree for
   identical state, and the popover permanently undercounts by the number of empty-result endpoints (31 of 119 outcomes
   were Empty in the capture).
2. Multi-cadence oscillation. `CharacterSkills` refreshes every 60s while Assets/Clones/Contacts/Profile/Wallet refresh
   hourly (`JobKind::interval()`). Skills perpetually cycles Done -> Syncing -> Done, so any instantaneous fraction
   bounces. A continuously-refreshing, multi-cadence system has no honest terminal "100%" when drawn as a finite bar.
3. In-memory-only status, no launch seeding (the worst). `SyncStatus` is in-memory per process and inits empty every
   launch. The engine emits a status event only when a job actually runs and finishes (`Event::Started`,
   `Event::Finished`, `Event::Scheduled` — the last emitted inside `finish()`). On launch the engine seeds the scheduler
   from `sync_ledger` (`seeds_for` / `future_seed`) so a fresh job is correctly parked up to an hour out, but enrollment
   emits no event. A fresh-on-disk, idle job therefore tells the UI nothing -> `phase == None` -> the cold default
   "Queued" with no countdown. Confirmed empirically: leaving the app open ~1 hour (the slowest cadence) made every job
   fire once and the display finally "completed" — proving the incompleteness is a cold-start artifact. The ledger
   already knew the truth at launch; the UI ignored it.

The data needed to fix this already exists: `sync_ledger` holds `last_success_at`, `next_eligible_at`, `outcome`, and
`last_reason` per (subject, kind); `needs_reauth` is a separate, authoritative flag on the credentials table that the
engine already honors (`engine.rs` enroll/dispatch) and the character cards already badge.

### Considered alternatives

- UI reads the ledger directly on launch. The app would query `sync_ledger::all()` + `needs_reauth` into a freshness
  snapshot consumed only by the chip/popover. Rejected: it adds a UI to DB read path that ADR-0002 deliberately avoids,
  leaves `SyncStatus` purely live so the character cards stay cold at launch (they read `SyncStatus`), and forces two
  parallel state models (snapshot + live events) to be reconciled in the view layer.
- Extend `Phase`/`SyncStatus` with new `Fresh` / `NeedsReauth` variants. Richest single vocabulary, and every consumer
  benefits. Rejected as the first move: `Phase` has the widest blast radius (54 affected symbols per the CodeGraph
  impact set), including the character-manager card/roster failure-badge logic and its tests. The freshness vocabulary
  is a view concern that can be derived without enlarging the shared enum.

## Decision

1. Add `Event::Seeded { key, outcome, next_in_secs }` to the sync event stream. The engine emits it once per enrolled
   job at launch, built from the `sync_ledger` rows it already reads in `seeds_for`. `SyncStatus::apply` gains a
   `Seeded` arm that maps the persisted outcome onto the existing `Phase` values (Synced/Empty -> Done/Empty,
   Blocked -> Blocked, Failed -> etc.) and records the next-run deadline — exactly as if a benign prior run had just
   reported in. No new `Phase` variant is introduced. A job flagged `needs_reauth` is seeded as Blocked so the
   attention path lights without special-casing.
2. The UI stays a pure observer. No component reads the database; the chip/popover continue to consume `SyncStatus`
   only. Seeding closes the cold-start gap at the source, so the existing `SyncStatus` readers — including the
   character-manager cards — show correct state at launch too.
3. One shared freshness derivation. A single function maps (`SyncStatus`, roster, enabled features, now) to the
   freshness vocabulary used by both surfaces, eliminating the `build_model` vs `job_stats` disagreement and the
   dead-code third definition in `status.rs`. Semantics:

   - Fresh — has a successful outcome (Synced or Empty) within its interval; shows its next-in countdown.
   - Refreshing — currently Syncing; rendered subtly and never changes the chip headline.
   - Catching up — never-succeeded-yet and enrolled (empty ledger / new character).
   - Needs attention — persistently failed, blocked, or needs re-auth (surfaced distinctly). Transient backoff/retry
     does not count — it self-heals and stays calm.
4. Chip headline precedence: N need attention > Catching up... N left > Up to date. Routine background refreshes show
   only as a subtle pulse; the words do not change while data remains fresh.
5. Align feature gating. The popover gates rows on the top-level `Feature` (`kind.feature()`) while the engine enrolls
   on sub-features (`kind.is_feature_enabled`). Align the row domain to the engine's gating so no row is permanently
   phantom-"Queued" for a job the engine will never service.

## Affected Areas

- `src/sync/event.rs` — new `Event::Seeded` variant.
- `src/sync/engine.rs` — emit Seeded from `seeds_for`/enrollment; fold the `needs_reauth` flag into the seeded state.
- `src/sync/status.rs` — apply arm for Seeded; reconcile/retire the dead-code count methods against the one shared
  derivation.
- `src/ui/components/sync_popover.rs` — disambiguated row states (replace bare "Queued"); feature-gating alignment; the
  shared derivation (or its call site).
- `src/ui/components/sync_chip.rs` — three-state, calm headline.
- `src/app.rs` — wiring `sync_model` / chip stats to the shared derivation.

## Consequences

- The cold-start "everything Queued for up to an hour" disappears; the surface is honest the moment the app opens.
- The "dance" is gone from the headline: a mid-refresh job no longer makes the chip look incomplete, because fresh data
  stays fresh while it refreshes.
- The character-manager cards inherit correct launch-time sync state for free (same `SyncStatus`, now seeded).
- `Phase` and its 54 dependents are untouched, keeping the change surface contained to the sync-status surface.
- One derivation is the single source of truth for the count; the chip and popover can no longer disagree.
- Cost: a new event variant and a one-shot burst of Seeded events at launch (bounded by enrolled job count); the engine
  already holds the ledger rows, so no extra query.

## Future Work

- If the freshness vocabulary proves valuable beyond the chip/popover, `Phase` could later gain explicit `Fresh` /
  `NeedsReauth` variants (the rejected alternative), migrating the derivation into `SyncStatus` itself. Deferred until a
  second consumer justifies the wider blast radius.

## References

- ADR-0002 — Sync/Render Separation (UI observes; sync owns execution).
- ADR-0014 — Persisted Sync Ledger and Honest Job Outcomes.
- Spec: gest artifact `ortpkouw` — Freshness-first sync status.
