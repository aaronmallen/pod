---
id: "0038"
title: Data Export/Import — Archive Format and Restore Strategy
status: active
tags: [storage, settings, sync, backup]
created: 2026-06-23
---

# ADR-0038: Data Export/Import — Archive Format and Restore Strategy

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Settings > Storage gains "Export data…" and "Import data…" actions so a user can move Pod's state to another machine
or keep a restorable backup. Export bundles a consistent snapshot of the live SQLite database plus `config.toml` into a
portable `.zip` archive; import validates that archive, backs up the current database, atomically restores the snapshot
into place, merges the archived settings while preserving this machine's identity, and then quits so the user reopens to
apply. The archive uses `zip` + `deflate` (not `.tar.zst`) reusing the existing log-export plumbing with zero new
crates, and the restore reuses the consistent-snapshot and atomic no-clobber replace machinery already in
`src/store/sync_copy.rs`. We explicitly reject `.tar.zst`, in-app live reload, partial export, and any CLI/MCP surface.

## Context

Pod's data — the SQLite database and `config.toml` — lives in per-machine directories (ADR-0007) with no built-in
way to move it to another machine or capture a restorable backup. Users who replace hardware, run Pod on more than one
machine, or want a safety net before a risky change have no supported path to capture and restore their state. The
Settings > Storage redesign introduces an Export/Import affordance to fill this gap.

Two problems are genuinely hard and must not be reinvented:

1. Capturing a consistent snapshot of a live, possibly syncing SQLite database. The working file has a `-wal`/`-shm`
   sidecar pair under WAL mode, and a naive copy of the main file alone can miss committed pages still in the WAL. The
   one-writer/many-readers access model (ADR-0031) and the networked-drive sync model (ADR-0016) already solved
   consistent capture: `sync_copy` checkpoints the WAL into a self-contained database file.
2. Replacing the current database atomically without clobbering a recoverable copy. ADR-0016 already defines a
   no-clobber, timestamped-backup publish step. Reusing it means a bad archive is always recoverable and a failed
   restore never leaves a half-written database.

Logs and the image cache are deliberately out of the picture: logs are machine-local observability (ADR-0012) and the
image cache is regenerable and already excluded from sync (ADR-0013), so neither belongs in a portable data archive.

## Decision

- Archive format — `.zip` with `deflate`, not `.tar.zst`. Export reuses the existing log-export zip plumbing and adds
  zero new crates (ADR-0033, minimal deps). The suggested filename is `pod-export-YYYY-MM-DD.zip`, saved to a
  user-chosen location via a file dialog.
- Archive layout. A single `.zip` containing:
  - `pod.db` — the database snapshot with its WAL folded in (self-contained, no `-wal`/`-shm` sidecars), taken from
    the live working file via the `sync_copy` checkpoint path.
  - `config.toml` — the user's settings.
  - `manifest.json` — a machine-parseable manifest carrying Pod version, schema/migration version, and OS/arch. This
    is the new piece import relies on for its version guard.
  - `MANIFEST.txt` — a human-readable manifest mirroring the existing log-export text manifest.
- Manifest-format divergence from log export. Log export writes only a plain-text `MANIFEST.txt`. Import must read
  `pod_version`/schema version programmatically for the version guard, so the data archive carries both a parseable
  `manifest.json` and the human `MANIFEST.txt`.
- Version guard. An archive from an older Pod restores fine — migrations run forward on next launch. An archive from a
  newer Pod (a higher schema version than this build supports) is refused with a clear message, because the schema
  cannot be downgraded.
- Restore = back-up-current-then-atomic-replace. On import, the current database is backed up (timestamped) and then the
  snapshot is published into place atomically via `sync_copy::publish_database`, reusing the existing backup/retention
  discipline so a bad archive is recoverable and a failure never leaves a half-written database.
- Sync-mode restore. The restored snapshot is written to the resolved canonical database path under the sync lease, then
  the `.generation` marker is bumped so the local working copy re-seeds from the canonical file on next launch. Import
  is refused (with a warning) while another machine holds the lease.
- Config merge preserving local identity. Portable settings (features, ui, accessibility, industry) are restored from
  the archive; this machine's path overrides (`db_dir`/`log_dir`/`cache_dir`), `machine_id`, MCP token, and
  `eve_client_id` are kept. This stops an import from pointing Pod at the source machine's paths or hijacking its sync
  identity.
- Quit-and-reopen to apply. There is no in-app live reload or automatic relaunch; import closes Pod so the user reopens
  to apply, matching the existing "takes effect on next launch" convention for storage changes.

## Affected Areas

- `src/store/sync_copy.rs` — reused for the consistent snapshot (checkpoint into a self-contained file) and the atomic
  no-clobber `publish_database` restore; Sync-mode generation/lease handling.
- Log-export plumbing — the zip writer and text-manifest format are reused; the data archive adds `manifest.json`
  alongside `MANIFEST.txt`.
- Settings > Storage UI — the Export/Import rows, file dialogs, the import confirmation warning, and the quit-to-apply
  flow.
- Config load/merge — identity-preserving merge of archived portable settings over local identity fields.

## Consequences

### Positive

- Reuses the already-hardened consistent-snapshot and atomic-replace machinery, so the hard correctness problems are not
  re-solved.
- Zero new crates; `.zip`/`deflate` reuses existing log-export plumbing (ADR-0033).
- Backup-first restore makes every import recoverable; the version guard prevents an unsupported schema downgrade.
- Identity-preserving config merge keeps a restored machine pointed at its own paths and sync identity.

### Negative

- The data archive carries two manifests (`manifest.json` + `MANIFEST.txt`), diverging from log export's single text
  manifest.
- Quit-and-reopen is a coarser UX than a live reload, but it matches existing storage-change behavior and avoids
  in-process database-swap hazards.
- Sync-mode restore adds lease/generation coordination that direct mode does not need.

## Open Questions

- Whether to surface a one-click "export before import" affordance in addition to the automatic backup.
- Retention policy for the timestamped pre-import backups (reuse the sync backup retention as-is initially).

## Future Work

- Selective/partial export (single character, date range) — explicitly out of scope here.
- An MCP or CLI export/import surface — out of scope; the feature is UI-only and no CLI exists.

## References

- Spec: gest `osqkxtoq` — Settings > Storage: Export and Import data.
- ADR-0007 — User-Configurable Storage Paths (path authority).
- ADR-0016 — Networked-Drive Storage-Sync Model (sync model; no-clobber timestamped backup).
- ADR-0031 — One-Writer/Many-Readers SQLite Access.
- ADR-0013 — Committed Item Icons and Synced Portraits/Logos (cache exclusion).
- ADR-0033 — Embedded MCP Server for Agent Automation (minimal deps / MCP boundary).
- Existing plumbing: `src/store/sync_copy.rs`, the log-export zip/manifest writer.
