---
id: "0007"
title: User-Configurable Storage Paths
status: active
tags: [architecture, config, storage]
created: 2026-06-06
---

# ADR-0007: User-Configurable Storage Paths

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The locations of Pod's two on-disk stores — the SQLite **database** and the **log** directory —
become **user-overridable settings**. Each has an optional override in `config.toml`; when set it
wins, otherwise the location is derived from the platform convention via `dir_spec` exactly as today.
This reverses the original `config.rs` stance that "paths are not user settings". When an override
changes, Pod **moves the existing store to the new location** and only repoints if the move is
impossible, with a clear failure path. The override does not change *what* the stores hold (ADR-0013
image layout and ADR-0003 app-owned tables are unaffected) — only *where* the roots live.

Pod keeps **no separate on-disk cache** — ESI responses and other transient data are cached in the
database itself — so the cache is not a relocatable store; only the database and logs are.

## Context

`config.rs` derives the data/database/log locations from platform conventions (`dir_spec::data_home`,
`state_home`) and its module docs state the data and database directories "are not user settings".
That was a deliberate simplification: one less thing to configure, and the platform convention is
correct for the overwhelming majority of users.

The Storage tab reverses that decision. Real users need to relocate these stores:

- **A large or growing store on a small system volume.** The image store (ADR-0013) grows with the
  item-render long tail and the database accumulates wallet-journal history (ADR-0003); a user may
  want all of it on a larger secondary drive.
- **Network or external volumes.** Pod already supports a database on a network-mounted filesystem
  (the `network` flag switches SQLite from WAL to DELETE journal mode), but offers no way to *point*
  the database at such a volume. The override is the missing half of that capability.

Once paths are user settings, two questions that did not exist before must be answered: how an
override resolves against the platform default, and what happens to the **bytes already on disk**
when a user changes a path. An empty override that silently abandons a populated database would be a
data-loss bug, so the relocation semantics are as much a part of this decision as the fields
themselves.

## Decision

Make the two store roots overridable, resolve override-over-default, and move-on-change.

### Override fields

Two optional path settings live in `Settings`, serialized in `config.toml`:

- **`db_dir`** — the directory holding `pod.db`.
- **`log_dir`** — the directory for log files.

Each is `Option<PathBuf>`: absent means "use the platform default". They are independent — a user may
override one and leave the other on convention. The **image store is not separately overridable**;
it remains rooted at `{data_dir}/images` (ADR-0013) and follows the data directory. Overriding the
database path moves the database; it does not move images.

### Resolution precedence

For each store, the effective path is resolved **override first, platform default second**:

```text
effective_path = settings.<override>.unwrap_or_else(|| dir_spec::<home>().join("pod")…)
```

The existing `dir_spec` derivation (`data_home` → database, `state_home` → logs) becomes the
*fallback* arm rather than the only arm. The current `config::data_dir` / `database_path` free
functions are extended to consult `Settings`, so every call site resolves the same way. There is
exactly one resolution per store, in `config.rs`; nothing reads a raw `dir_spec` home directly for
these stores.

### Relocation / move semantics

Changing an override is a **relocation**, not merely a repoint. When the user applies a new path for
a store that already exists on disk:

1. **Move existing files by default.** Pod moves the current store to the new root (a filesystem
   rename when the source and destination are on the same volume; a copy-then-delete when they are
   not). The database move includes its WAL/SHM sidecars (or the DELETE-mode journal); the log
   directory moves as a tree.
2. **Repoint-only when there is nothing to move.** If the old location is empty/absent (e.g. a
   fresh install, or a store that has never been written), there is nothing to relocate and Pod
   simply records the new path — the store is created there on next use.
3. **Atomicity and failure handling.** The move is staged so a failure never strands the user
   between two half-populated locations:
   - The database connection is **closed/quiesced before** the move so no handle is open across the
     rename.
   - On a **cross-volume copy**, the source is deleted only after the copy fully succeeds; a partial
     copy is rolled back (the partial destination is removed) and the **override is not committed**,
     so Pod keeps using the old, intact location.
   - If the destination is **non-writable, non-existent and uncreatable, or out of space**, the
     change is rejected with a surfaced error and the setting reverts to the prior value.
   - The override is **persisted only after** the move (or repoint) succeeds, so `config.toml` never
     names a location the data is not actually at.

A relocation requires reopening the affected store at the new path. The database reopen re-runs the
ADR-0002 connection setup (including the network/WAL decision below); the image store is re-rooted at
its new directory.

### Interaction with the network-drive / WAL flag

The `network` flag (DELETE vs WAL journal mode) and the `database_path` override are
**complementary and resolved together at open time**. The override decides *where* the database file
lives; the `network` flag decides *how* SQLite journals it there. After a relocation the database is
reopened, and the journal mode is selected for the **new** location's volume — so moving the
database onto a network share is exactly the scenario the `network` flag exists for, and the two
settings are expected to be set in the same Storage-tab interaction. The override never silently
changes the journal mode; that remains the `network` flag's job.

### Relationship to ADR-0013 (Images) and ADR-0003 (Canonical Data Model)

- **ADR-0013 (Image assets).** Image layout — derived paths under `{data_dir}/images`,
  presence-on-disk as source of truth, atomic temp-then-rename writes — is **unchanged**. Only the
  `{data_dir}` root can move (with the database, as part of the data directory). The render layer
  still resolves paths (portraits/logos) relative to the resolved root, so a missing file remains a
  re-sync trigger, not a placeholder.
- **ADR-0003 (App-owned data, Canonical Data Model).** App-owned tables (squads, and later tags/skill plans/fittings)
  live
  in the same `pod.db`, so they relocate **with the database** as one unit — there is no separate
  app-owned store and nothing extra to move. The override changes the file's location, never its
  schema, write paths, or sync exclusion.

This ADR also **amends the earlier premise** that storage paths are fixed: paths are now user
settings, but everything ADR-0013 and ADR-0003 say about *content and layout within* a root still
holds.

## Affected Areas

- `src/config.rs` — `Settings` gains `db_dir`/`log_dir` overrides; `data_dir`, the database path, and
  the log resolver consult `Settings` (override-over-default) instead of reading `dir_spec` directly.
  The "paths are not user settings" module doc is corrected.
- `src/store.rs` — relocation helper that quiesces, moves (rename or copy-then-delete with rollback),
  and reopens the database at a new path, threaded with the existing `network`/WAL `open` path.
- `src/store/images.rs` — image store re-rooted off the resolved `data_dir` (no layout change).
- Logging init — reads its directory from the resolved settings rather than the raw platform home.
- The Storage tab — surfaces the two overrides, the `network` flag, and the move-vs-repoint outcome /
  errors of an apply.

## Consequences

### Positive

- Users can place the database and logs where their hardware and backup strategy want them, including
  network and external volumes (pairing naturally with the `network`/WAL flag).
- One resolution rule per store (override-over-default) keeps every call site consistent; the
  platform convention remains the zero-config default.
- Move-on-change means relocating a store does not abandon or duplicate the data already in it, and a
  failed move leaves the original intact rather than stranding the user.

### Negative

- Resolving a store path now depends on `Settings`, so the previously pure `config::data_dir` /
  `database_path` functions take (or close over) configuration — call sites and tests must supply it.
- Relocation is a genuinely stateful operation (close handles, move bytes, reopen) with cross-volume
  and out-of-space edge cases; it is more code and more failure modes than a fixed path.
- A user can point a store at an unsuitable location (removable drive that is unmounted at next
  launch, a synced cloud folder fighting SQLite locking); Pod validates writability on apply but
  cannot prevent the environment changing afterward.

## Future Work

- A separate image-store override, if users want images apart from the database (today they move
  together with the data directory).
- A "reset to platform default" affordance in the Storage tab that relocates a store back to the
  convention.
- Detection and recovery UX for a configured path that is missing at launch (e.g. an unmounted
  volume), rather than failing to open.

## References

- ADR-0002 — Sync/Render Separation and Aggregation Chaining (`0002-sync-render-separation.md`)
- ADR-0013 — Image Assets — Committed Item Icons and Synced Portraits/Logos (`0013-committed-item-icon-set.md`)
- ADR-0003 — Canonical Data Model (`0003-canonical-data-model.md`)
