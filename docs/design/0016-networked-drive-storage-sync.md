---
id: "0016"
title: Networked-Drive Storage-Sync Model
status: active
tags: [architecture, storage, sync]
created: 2026-06-10
---

# ADR-0016: Networked-Drive Storage-Sync Model

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod can keep its database on a network share so the same data follows a user across machines. The original
shared-drive feature made this usable but shipped a guaranteed data-loss path, conflated two distinct storage
locations, and never synced automatically in both directions. This ADR records the reworked model: **two cleanly
separated locations** — a user-configurable *shared canonical* save file (which may live on a network share) and an
*always-local working copy* (the live SQLite DB + WAL, never redirectable onto a network FS) — entered by **explicit
opt-in only**, kept safe by a **no-clobber timestamped-backup invariant** that makes overwriting real data without a
backup structurally impossible, seeded and reconciled safely at launch, and synchronized **bidirectionally and
automatically** under a single-writer lease with last-writer-wins generation markers. No database schema changes: the
`.generation` sidecar markers and `lease.json` stay as-is.

## Context

The shipped model treated the live DB as a working copy and the user-configured location as the canonical "save
file", gating pull/push on `.generation` sidecar markers. Several defects made it unsafe and incomplete:

- **Data loss on auto-detected upgrade.** Mode selection flipped to Sync whenever the data dir sat on a network FS,
  even with the opt-in flag off. A pre-sync DB has no `.generation` sidecar, so the boot path saw generation `0 <= 0`,
  pulled nothing, opened an empty working copy, and on clean exit copied that empty copy over the real canonical with
  no backup. The safe seeding logic existed only on the explicit settings toggle, never on the boot-time flip:
  bootstrap and migration disagreed.
- **The working copy could land on the network.** It derived from the user-configurable, evictable cache dir, so
  pointing the cache at a NAS put the live DB — the thing that most needs fast local disk and WAL — on the network,
  and cache eviction could delete it.
- **"Sync now" was a silent no-op** whenever a stale or foreign lease was held.
- **No automatic pull.** Push was periodic; another machine's changes only arrived at boot or on take-over, so it was
  not really sync.
- **Already-diverged installs were stranded** with a canonical and a separate, newer working copy that disagreed, and
  nothing reconciled them.

These are durable architectural invariants — where the bytes live, who may write, and how data-loss is prevented — so
they warrant a recorded decision rather than living implicitly in the bootstrap and migration code.

## Decision

### Two cleanly separated locations

- **Shared canonical location** — the synced save file. User-configurable, MAY be a network share, defaults to
  `dir_spec::data_home()`. This is the only network-capable configurable path, and what "sync this location across
  machines" targets. `resolved_database_path()` is unchanged.
- **Local working copy** — the live DB (+ WAL). ALWAYS on guaranteed-local, non-evictable disk, derived from a
  dedicated base (`state_home/pod/db`) that is independent of the configurable cache dir. The override that selects it
  is internal-only (never persisted to `config.toml`, never surfaced in the UI), and a network-FS guard redirects the
  working copy to a local fallback if its base is ever classified as network. A user cannot place the live DB on a
  network share.

### Opt-in-only Sync, detection as advisory

Entering Sync mode is driven **solely** by the explicit network opt-in flag. Network-FS detection never flips the mode;
it becomes an advisory suggestion ("this looks like a network share — enable syncing across machines") surfaced in
Settings, and the toggle is always interactive so a misdetected share can still be turned off. Enabling Sync routes
through the existing safe migration path.

### No-clobber timestamped-backup invariant

A single guarded "publish/replace database" primitive is the only way any site overwrites a database file: before
replacing a destination that exists and is non-empty, it first writes a timestamped `.backup` of that destination;
replacing a missing or empty destination needs no backup. Every replace site — push, boot-adopt, and
migration/reconcile — routes through it. This directly closes the gen-0-vs-0 hole: real data is never overwritten
without a recoverable copy. It is the same "back up, don't wipe" philosophy Pod applies to breaking on-disk upgrades.

### Safe boot seeding and launch-time reconcile

Any path that reaches Sync mode adopts an existing canonical DB rather than opening empty: when the working copy is
absent or empty and the canonical holds real data, the working copy is seeded from the canonical and the markers are
brought in step. First launch of the fixed version also detects orphaned or diverged state — a canonical plus a
separate non-empty working copy whose markers disagree, or leftover working-copy/lease/sidecar artifacts while now in
Direct mode — adopts the newer/non-empty copy as truth, backs up the other (via the primitive above), converges the
markers, and cleans up stray Sync artifacts in Direct mode.

### Bidirectional automatic sync under a lease

A single-writer **lease** (`lease.json`) coordinates which machine owns writes. The holder pushes local changes to the
share periodically (dirty-gated) and the model **adds periodic pull**: when the share generation has advanced and no
local write is in flight, remote changes are applied automatically, respecting the lease / read-only state.
Conflicts resolve **last-writer-wins** by generation marker — the bytes always land before the generation bumps, so a
crash mid-publish understates the generation (re-pushed next time) rather than overstating it (which would skip a
needed pull and lose data). "Sync now" runs a real push+pull reconcile and reports its outcome (last-synced time,
"nothing to sync", or an error), reclaiming a stale lease or explaining that another machine holds it instead of
silently doing nothing.

## Affected Areas

- `src/config.rs` — opt-in-only `storage_mode`, the advisory `suggests_network_sync`, and the local-only working-copy
  derivation with its network-FS guard.
- `src/store/sync_copy.rs` — the `publish_database` no-clobber primitive and push/pull.
- `src/store/bootstrap.rs` — safe Sync seeding and launch-time divergence/orphan reconcile.
- `src/store/sync_session.rs` — the lease and bidirectional automatic sync.
- `src/store/storage_migration.rs` — Direct↔Sync transitions routing through the backup primitive.
- `src/features/settings/storage_tab.rs` — always-interactive toggle and the advisory banner.

## Consequences

### Positive

- Overwriting real data without a recoverable backup is structurally impossible.
- The live DB is always on fast local disk and can never be redirected onto a network share or evicted with the cache.
- Sync is genuinely bidirectional and automatic, and "Sync now" is honest about what it did.
- Already-diverged installs (and the dev environment) are rescued on first launch of the fixed version.

### Negative

- Routing every push through the backup primitive accumulates timestamped `.backup` files over time; pruning / rolling
  retention is deferred to follow-up work.
- The working copy moves from the old cache-derived location to the local base; the one-time adoption of an existing
  cache working copy is handled by the launch-time reconcile.
- Last-writer-wins discards a losing concurrent edit (preserved as a backup); Pod does not attempt row-level merge.

## Future Work

- Retention/pruning of accumulated `.backup` files.
- Surfacing richer sync status (in-flight pull/push, conflict notifications) in the UI.

## References

- [ADR-0007: User-Configurable Storage Paths](0007-user-configurable-storage-paths.md) — the configurable path roots
  this model separates into canonical (network-capable) vs working copy (local-only).
- [ADR-0014: Persisted Sync Ledger and Honest Job Outcomes](0014-persisted-sync-ledger-and-honest-outcomes.md) — the
  honesty contract the "Sync now" feedback follows.
- Pod's "back up, don't wipe" philosophy for breaking on-disk upgrades — the same invariant generalized here into the
  no-clobber publish primitive.
