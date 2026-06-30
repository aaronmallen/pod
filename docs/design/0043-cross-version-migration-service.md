---
id: "0043"
title: Cross-Version Migration Service
status: active
tags: [migration, boot, config, storage]
created: 2026-06-30
---

# ADR-0043: Cross-Version Migration Service

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Cross-version healing, meaning the version-to-version data and config fixups that are distinct from sqlx schema
migrations, is centralized into a single migration service (`src/services/migration.rs`) built around an imperative,
per-version `Migrator` trait. At boot the service determines the version being upgraded from, then runs each applicable
migrator in version order, bracketing the sqlx database migration with `before_db_migration` and `after_db_migration`
hooks.

## Context

Whenever a release breaks compatibility with on-disk state (config schema, DB checksums, where a value is stored), the
fix has historically been bolted on wherever was convenient: a CRLF migration-checksum healer inside `store::open`,
serde-time config healers in `config.rs`, and a half-built config-to-DB facility move wired only into tests. There is no
single, discoverable place to express "when upgrading from version X, do Y," and no consistent contract for when such
fixups run relative to the database migration. As the app accrues more cross-version state changes, this scattering
becomes a maintenance and correctness hazard (for example ordering relative to `migrator.run`, idempotency, and
fresh-install detection).

## Decision

Introduce `src/services/migration.rs` exposing an imperative `Migrator` trait:

- `fn version(&self) -> semver::Version`: the release the migrator targets.
- `async fn before_db_migration(&self, pool: &SqlitePool) -> Result<()>`: default no-op, runs on the writer pool
  immediately before the sqlx migrate.
- `async fn after_db_migration(&self, db: &Database, config: &mut Settings) -> Result<()>`: default no-op, runs
  immediately after the sqlx migrate, with the opened `Database` and config available.

A registry runs every migrator where `from < version <= current`, in ascending version order, during the splash Loading
phase (after the update-check resolves), bracketing the existing sqlx migration. The "from" version is parsed from the
`pod-X.Y.Z` token embedded in the existing `sde_version` marker, read before the seed task rewrites it. Fresh installs
(no marker, no DB) skip all migrators, and pre-marker installs with an existing DB floor to `0.6.0`. Every migrator must
be idempotent. The typed config-struct chain (`ConfigMigration<Old>` plus `enum_dispatch`) was rejected because the real
cases are heterogeneous side effects (DB checksum repair, config-to-DB data move), not a single config struct evolving.

## Affected Areas

- `src/services/migration.rs` (new) and `src/services.rs` (module decl).
- `src/store.rs` / `store::open_pools`: exposes the seam so the before-hook fires before `migrator.run`.
- `src/app/boot.rs`: drives the runner in the Loading-phase store-open worker, supplying the after-hook with `Database`
  and config.
- `src/store/migration_checksum_repair.rs`: relocated behind the `before_db_migration` hook (the 0.6.7 migrator).
- `src/config.rs`, `src/store/repo/industry.rs`, and `src/features/settings/facility_tab.rs`: the facility config-to-DB
  migrator and dual-write retirement.
- The serde-time config healers in `config.rs` are explicitly out of scope and remain tolerant-load healers.

## Dependencies

| Dependency  | Version | Purpose                                                                                                                   |
|-------------|---------|---------------------------------------------------------------------------------------------------------------------------|
| `toml_edit` | latest  | Comment-preserving surgical edits to the on-disk config file (the `toml::to_string_pretty` save path drops user comments) |

## Consequences

### Positive

- One discoverable home and a uniform contract for cross-version fixups.
- Clear ordering guarantees relative to the sqlx migration (the before and after hooks).
- Converts two existing ad-hoc cases (CRLF repair, facility move) into first-class, testable migrators.
- No new schema and no new marker file, reusing the existing `sde_version` signal.

### Negative

- A new architectural pattern (trait-based dispatch) the codebase did not previously use.
- From-version detection leans on the `sde_version` token advancing on version bumps, so idempotency is required as a
  safety net if a reseed is skipped or fails.
- Adds a dependency (`toml_edit`).

## Open Questions

- Failure policy when a migrator errors: abort boot (`InitFailed`) versus log-and-continue. Leaning toward abort.
- Whether migration surfaces splash progress labels or runs silently.

## Future Work

- Aligning the minimum supported version concept with the parked sqlx-migration consolidation (0.7.0), which needs a
  min-version gate plus a `_sqlx_migrations` restamp.
- Migrating the serde-time config healers into the service, if a uniform model proves valuable.

## References

- Spec: Centralized Cross-Version Migration Service (gest `lvzozomp`).
- Related ADRs: [0031] (one-writer / many-readers SQLite access model).

[0031]: 0031-one-writer-many-readers-sqlite-access.md
