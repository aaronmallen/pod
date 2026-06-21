---
id: "0031"
title: One-Writer / Many-Readers SQLite Access Model
status: active
tags: [persistence, performance, sqlite, concurrency]
created: 2026-06-21
---

# ADR-0031: One-Writer / Many-Readers SQLite Access Model

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The app accesses its single SQLite database through one [`Database`](../../src/store.rs) handle that owns a **single
dedicated writer connection** (a `max_connections = 1` pool) and a **multi-connection reader pool**. Every write —
`.begin()` transactions and direct `execute` mutations — routes through `Database::writer()`; every read (`fetch_*`)
routes through `Database::reader()`. This matches SQLite's own WAL constraint (single-writer / many-readers) exactly, so
read-only work is physically immune to write-storm starvation. The previous three write-capable pools
(interactive = 4, sync = 4, housekeeping = 1) over one WAL file are gone; `open_pools` now returns three handles that
all clone the same reader pool and writer connection.

## Context

SQLite in WAL mode allows any number of concurrent readers but exactly **one** writer at a time. The app nevertheless
ran three independently write-capable pools against one WAL file: an interactive pool for auth/roster reads, a sync
worker pool, and a single-connection housekeeping pool. The intent was isolation — keep interactive reads off the sync
workers' connections — but it does not hold under WAL: a sync write-storm (the per-character sync writing journals,
transactions, assets, ledger rows in tight succession) takes the one WAL write lock, and any other pool that needs to
write, or even just to acquire a connection that then contends, queues behind it.

The concrete failure (epic `vuzvqqvp`): on cold open the read-only roster load (`load_roster_at`) would block until it
hit sqlx's **default 30s `acquire_timeout`** and return `Err`, surfacing as "Couldn't load characters" and empty views
until the user cycled the UI. Two further misconfigurations compounded it:

- `cache_size = -262144` (256 MB **per connection**, not shared). With up to nine write-capable connections this could
  reserve gigabytes of resident page cache.
- `min_connections = 0` plus sqlx's ~10-minute idle reaping caused connections to be torn down and re-established in
  bursts — the source of the observed ~94-connection WAL-pragma storm at startup/idle.

## Decision

Encode SQLite's constraint directly in the access model.

- **One writer.** `Database` holds a writer pool capped at `max_connections = 1`. Migrations and every mutation run on
  it. There is no second writer to contend for the WAL write lock, so serialization happens at the pool (cheap, in
  process) instead of at the file lock (a `busy_timeout` spin).
- **Many readers.** `Database` holds a reader pool (`READER_MAX_CONNECTIONS = 8`) shared by interactive and sync reads.
  WAL readers never take the write lock, so they are immune to write-storm starvation by construction.
- **`open_pools` collapses to one `Database`.** The `interactive` / `sync` / `housekeeping` fields are retained for
  call-site clarity but are clones of the same handle. Housekeeping keeps its intent (low-priority maintenance) within
  the model: its writes go through the shared writer, its reads through the shared reader pool.
- **Fail-fast contention.** Both pools set an explicit `acquire_timeout = 5s` (down from the 30s default) so any genuine
  contention surfaces as a fast, recoverable `Err` the read path can retry/degrade on, never a multi-second UI freeze.
- **Right-sized, warm connections.** `cache_size` drops to 48 MB/connection (`-49152`); the reader pool warms
  `min_connections = 2` and the writer `min_connections = 1`, kept alive for the process lifetime so the cold-open
  roster read pays no connection-establishment latency and idle reaping never triggers a reconnect burst. WAL +
  `synchronous = NORMAL` + `foreign_keys` are preserved.

Call sites route by operation: `db.writer()` for `.begin()` and `execute`, `db.reader()` for `fetch_*`. The tuple fields
`.0` (reader) and `.1` (writer) back the accessors; the accessors are preferred for readability.

### Chosen defaults

| Setting             | Value          | Rationale                                                                                                              |
|---------------------|----------------|------------------------------------------------------------------------------------------------------------------------|
| Reader pool size    | 8              | Covers interactive reads + sync reads concurrently without queueing; bounds resident cache.                            |
| Writer pool size    | 1              | Matches SQLite's single-writer WAL constraint exactly.                                                                 |
| Reader warm (`min`) | 2              | Cold-open roster read hits a live connection; no reaping.                                                              |
| Writer warm (`min`) | 1              | Writer is created once, pinned, never reaped.                                                                          |
| `acquire_timeout`   | 5s             | Fail fast instead of the 30s default hang.                                                                             |
| `busy_timeout`      | 5s             | Aligned with `acquire_timeout`.                                                                                        |
| `cache_size`        | -49152 (48 MB) | Keeps large seeds/roster reads in memory; bounds worst-case resident cache to a few hundred MB across the reader pool. |

## Consequences

- The read-only roster load can no longer block on the write lock or hit the acquire timeout under sync write pressure —
  verified by `store::tests::open::it_serves_reads_under_simulated_sync_write_pressure`, which runs 50 reads against an
  ongoing 200-transaction write-storm and asserts each read completes inside the acquire timeout.
- Writes across the whole process serialize through one connection. This is not a regression: WAL already serialized
  them at the file lock; serializing at the pool is cheaper and more predictable, and the housekeeping pool's original
  purpose (a guaranteed free slot for maintenance writes) is subsumed — there is one writer slot and it is always the
  same one.
- No schema migration is required; this is purely an access-layer change.
- Resident memory at idle drops sharply (no oversized per-connection cache, no idle reconnect bursts).
