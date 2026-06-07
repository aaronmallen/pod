---
id: "0012"
title: Logging and Observability Conventions
status: active
tags: [architecture, observability, logging, tracing, conventions]
created: 2026-06-06
---

# ADR-0012: Logging and Observability Conventions

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod's instrumentation is unified under a single set of conventions. Application events are emitted through `tracing`
under a fixed **namespace taxonomy** — `pod::nav`, `pod::ui`, `pod::http`, `pod::lifecycle`, `pod::sync` — each with a
defined level policy and a shared vocabulary of **structured fields** (`character_id`, `route`, `elapsed_ms`, `status`,
and the dispatched `Message` variant name). The **query channel is not a Pod namespace**: SQL is logged through sqlx's
own native `sqlx::query` target rather than a hand-rolled `pod::query` wrapper, because sqlx already instruments all 379
direct call sites across the 35 repo files for free. Logs fan out to **two sinks** — a compact ANSI text layer to the
console at a readable default, and a JSON-per-line layer to a daily-rolling file at `TRACE` for `pod` targets — with
explicit third-party **noise floors** on the file layer and a carve-out that keeps `sqlx::query` flowing to the file so
query logging survives. The non-blocking file appender's `WorkerGuard` is held for the whole program lifetime so
buffered logs flush on exit.

This ADR is the single source of truth that the instrumentation-chokepoint tasks (navigation, message dispatch, HTTP,
lifecycle, sync) implement against. They consume these conventions; they do not re-decide them.

## Context

Today `init_tracing` in `src/app.rs` installs one `tracing_subscriber::fmt` layer with the env filter
`warn,pod=debug` and writes only to the console (`src/app.rs` around line 205, called from `run()` around line 192).
There is no file sink, no structured-field discipline, and no namespace convention: existing call sites use bare
messages under the crate's default target (32 `debug`, 35 `warn`, 9 `error`, 7 `info`, 1 `trace`), so an event's
channel and severity are ad hoc per author.

The logging rework introduces several instrumentation **chokepoints** — navigation, `update()` message dispatch, the
HTTP client, sync jobs, and process lifecycle — each implemented by a separate task. Without a documented convention,
those tasks will independently invent namespaces (`pod::http` vs `pod::net` vs `pod::esi`), pick levels inconsistently
(is a route change `info` or `debug`?), and name the same datum five different ways (`char_id`, `character`, `cid`). The
result is logs that cannot be filtered, correlated, or read across subsystems. The convention must exist **before** the
chokepoint tasks land so they all target the same shape.

Two forces specifically shape the design:

- **SQL is already instrumented.** sqlx emits a structured `tracing` event on its `sqlx::query` target for every query
  it runs — full SQL, elapsed time, and rows affected/returned — across all 379 direct sqlx call sites in the 35
  `src/store/repo/` files. Hand-rolling a `pod::query` wrapper to duplicate that would mean touching every one of those
  call sites, would drift from the actual SQL sqlx executes, and would add nothing sqlx does not already give us. So the
  query channel is sqlx's, not Pod's.
- **Logs must survive crashes and exits.** A GUI app (Iced daemon) can exit abruptly. A buffered, non-blocking file
  appender drops in-flight lines unless its background worker is explicitly flushed, which `tracing-appender` ties to
  the lifetime of a `WorkerGuard`. Where that guard lives is therefore a correctness decision, not an implementation
  detail.

The log **directory** is already a resolved, user-overridable setting (ADR-0007): logging init reads
`Settings::storage().resolved_log_dir()` (`src/config.rs`), which honors a `[storage] log_dir` override and otherwise
falls back to `{data_dir}/logs`. This ADR builds the file sink on top of that resolved path; it does not re-decide where
logs live.

## Decision

### 1. Namespace taxonomy

Application events are emitted under a fixed set of dotted `tracing` **targets**, each owning one subsystem:

| Namespace        | Owns                                                                                  |
| ---------------- | ------------------------------------------------------------------------------------- |
| `pod::nav`       | Route/navigation changes — the user moving between screens and the resolved `route`.  |
| `pod::ui`        | `update()` message dispatch — each `Message` variant handled by the Iced update loop. |
| `pod::http`      | The HTTP/ESI client — outbound requests, responses, retries, rate-limit waits.        |
| `pod::lifecycle` | Process and store lifecycle — boot, store open, runtime build, shutdown.              |
| `pod::sync`      | Sync engine and jobs — scheduling, job start/finish, dependency-chain triggers.       |

These are `tracing` targets (the string after `target:` in an event), set explicitly so the channel is independent of
the emitting module path. A chokepoint task emits on exactly one of these targets; it does not introduce a sibling
namespace without amending this ADR.

The **query channel is deliberately absent** from the table above. It is **sqlx's native `sqlx::query` target**, which
carries the full SQL statement, elapsed time, and row count for every query. We do **not** wrap database access in a
`pod::query` target. Rationale: sqlx already instruments all **379 direct sqlx call sites across 35 repo files**
(`src/store/repo/`); duplicating that as a Pod namespace would require editing every call site, would inevitably diverge
from the SQL sqlx actually runs, and would add no information sqlx does not already emit. The query channel is consumed
**as-is** from sqlx and merely *routed* (see §5), not re-implemented.

### 2. Per-namespace level guidelines

Levels are chosen so the console default (`info` and up for `pod`, see §4) shows the meaningful skeleton of what the app
is doing, while `debug`/`trace` carry the detail that only matters when investigating.

- **`pod::nav`**
  - `info` — a route actually changes (the user navigates to a new screen).
  - `debug` — navigation attempts that resolve to the current route (no-op re-selects), back/forward bookkeeping.
  - `warn` — navigation to an unresolvable/invalid route.
- **`pod::ui`**
  - `trace` — every `Message` variant entering `update()` (high-volume; off by default at console).
  - `debug` — notable state transitions resulting from a message (popover toggles, selection changes).
  - `warn` — a message handled in an unexpected state, or a dropped channel send.
- **`pod::http`**
  - `debug` — each request issued and its response `status` + `elapsed_ms`; cache hits.
  - `info` — coarse milestones worth seeing without `debug` (e.g. a paginated fetch completing N pages), used sparingly.
  - `warn` — retries, rate-limit / error-limit waits (ESI `X-ESI-Error-Limit-Reset`), non-2xx that is recovered.
  - `error` — a request that ultimately fails after the client's retry policy.
- **`pod::lifecycle`**
  - `info` — boot, store opened, runtime built, clean shutdown — the major phase transitions.
  - `debug` — sub-steps within a phase (resolved paths, journal-mode selection).
  - `error` — store open / runtime build failure (these already exist as `tracing::error!` in `run_open_store` and
    `run_build_runtime`).
- **`pod::sync`**
  - `info` — a job starting and finishing, and dependency-chain triggers (ADR-0002).
  - `debug` — scheduling decisions, due-now computation, per-subject lane detail.
  - `warn` — a job skipped (e.g. missing ownership/grant), a transient failure that will be retried.
  - `error` — a job that fails terminally for the cycle.

`trace` is reserved for the highest-volume, per-event firehose (notably every `pod::ui` message); it is never the
default-visible level. `error` is for failures the user or operator must know about; `warn` is for recovered or expected
degradations.

### 3. Standard structured fields

Events attach data as `tracing` **structured fields**, not string interpolation, so the JSON sink can index them.
A shared vocabulary keeps the same datum named the same way everywhere:

| Field               | Type         | Applies to                                                                                       | Meaning                                                                                                       |
| ------------------- | ------------ | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------- |
| `character_id`      | `i64`        | any event scoped to a character (`pod::sync`, `pod::http` ESI calls, character-scoped `pod::ui`) | The EVE character the event concerns.                                                                         |
| `route`             | `string`     | `pod::nav` (and `pod::ui` where relevant)                                                        | The resolved route, rendered as its canonical name.                                                           |
| `elapsed_ms`        | `u64`/`f64`  | timed operations (`pod::http` requests, `pod::sync` jobs)                                        | Wall-clock duration of the operation in milliseconds.                                                         |
| `status`            | `u16`/string | `pod::http` responses; `pod::sync` job outcomes                                                  | HTTP status code, or a job's success/skip/error status.                                                       |
| `message` (variant) | `string`     | `pod::ui`                                                                                        | The `Message` enum **variant name** being dispatched (e.g. `"Sync"`, `"WindowOpened"`), not the full payload. |

Rules:

- Fields are emitted as `tracing` key–values (`character_id = id, %route, …`), never folded into the message text.
- A field is present **only when it applies** — a `pod::lifecycle` boot event has no `character_id`; a cache-hit
  `pod::http` event has no `elapsed_ms` for a network round-trip. Absent ≠ null; omit the field.
- The `Message` variant is logged by **name only**. Full payloads can be large, may contain bulk data, and add noise;
  the variant name is enough to follow the dispatch sequence.
- `character_id` is always the numeric EVE id, so it correlates across `pod::sync`, `pod::http`, and `pod::ui` for the
  same character.

### 4. Dual-sink subscriber

`init_tracing` installs a `tracing_subscriber::Registry` with **two layers**, each with its own filter and format:

- **Console (text) layer** — a compact, human-readable ANSI `fmt` layer to stderr/stdout, filtered at a **readable
  default**: third parties at `warn`, `pod` (and `pod::*`) at `info`. This is the "watch the app run" view; it
  intentionally omits the `pod::ui` `trace` firehose and the raw `sqlx::query` stream. The env var
  (`RUST_LOG`/`EnvFilter::from_default_env`) overrides this default when set, preserving today's behavior.
- **File (JSON) layer** — a `fmt` layer in **JSON mode** (`.json()`), one object per line, written to a
  **daily-rolling** file via `tracing_appender::rolling::daily(resolved_log_dir, "pod.log")` wrapped in
  `tracing_appender::non_blocking(...)`. This layer is filtered at **`TRACE` for `pod` targets** so the file is the
  complete record — every navigation, message, request, job, and (via §5) every query — for after-the-fact diagnosis.
  The log directory is `Settings::storage().resolved_log_dir()` (ADR-0007); init does not read a raw platform home.

One object per line keeps the JSON file `jq`/`grep`-friendly and append-safe under the rolling appender. The two layers
share one registry, so a single event is formatted once per sink according to that sink's filter.

### 5. Third-party noise floors and the `sqlx::query` carve-out

The **file layer** would otherwise drown in dependency chatter at `TRACE`. The file layer therefore applies per-target
**noise floors** that raise the minimum level for noisy third parties while keeping `pod` at `TRACE`:

| Target                                  | File-layer floor           |
| --------------------------------------- | -------------------------- |
| `pod`, `pod::*`                         | `trace`                    |
| `sqlx::query`                           | `debug` (kept — see below) |
| `sqlx` (non-`query`, e.g. pool/connect) | `info`                     |
| `hyper`                                 | `info`                     |
| `reqwest`                               | `info`                     |
| `iced`                                  | `info`                     |
| `wgpu` (and `wgpu_*`)                   | `info`                     |

The **explicit exception** is `sqlx::query`: it is **kept routed to the file** (at `debug`, the level sqlx emits its
per-query events) even though the broader `sqlx` target is floored to `info`. This is what makes the file the system of
record for SQL — full statement, `elapsed`, and rows — fulfilling the query-channel decision in §1 without any Pod-side
wrapper. The floor on the *generic* `sqlx` target silences pool/acquire/connection bookkeeping; the carve-out on
`sqlx::query` preserves the actual query log. Getting this directive order right (general `sqlx=info`, then specific
`sqlx::query=debug`) is essential — invert it and query logging disappears.

These floors live on the file layer's `EnvFilter`. The console layer's own default (third parties at `warn`, `pod` at
`info`) already excludes this dependency chatter, so the floors are primarily a file-layer concern.

### 6. WorkerGuard lifetime

`tracing_appender::non_blocking` returns a `(NonBlocking, WorkerGuard)` pair. The `NonBlocking` writer hands log lines
to a background worker thread; **dropping the `WorkerGuard` flushes and joins that worker**. If the guard is dropped
early
(e.g. left as a `_` temporary inside `init_tracing`), the worker is torn down immediately and **buffered lines written
after init are lost** — and on a normal or abrupt exit the tail of the log never reaches disk.

Therefore the `WorkerGuard` is **held for the entire program lifetime**. `init_tracing` returns the guard to `run()`
(`src/app.rs` around line 192), which keeps it alive in a binding that lives as long as the Iced daemon runs (i.e. until
`run()` returns). The guard must **not** be `let _ = …`-dropped or scoped to `init_tracing`. Holding it for the whole
program is what guarantees the non-blocking file sink flushes on exit, including the lifecycle "shutdown" event itself.

## Affected Areas

- `src/app.rs` — `init_tracing` (around line 205) is rewritten from a single console `fmt` layer into the two-layer
  registry of §4–§5; it returns the `WorkerGuard`. `run()` (around line 192) holds that guard for the daemon's
  lifetime (§6). The existing `tracing::error!` sites in `run_open_store` / `run_build_runtime` become `pod::lifecycle`
  events.
- `src/config.rs` — no change required by this ADR; init consumes the already-resolved
  `Settings::storage().resolved_log_dir()` (ADR-0007) for the file sink's directory.
- `src/clients/http.rs` — the HTTP client is the `pod::http` chokepoint: requests/responses log `status`, `elapsed_ms`,
  and `character_id` per §2–§3 (separate task).
- Navigation, `update()` dispatch, and the sync engine/jobs — the `pod::nav`, `pod::ui`, and `pod::sync` chokepoints,
  emitting per §2–§3 (separate tasks).
- `src/store/repo/` — **unchanged**: the 379 sqlx call sites across 35 files emit their query logs via sqlx's native
  `sqlx::query` target; no Pod wrapper is added (§1).

## Dependencies

| Dependency           | Version          | Purpose                                                              |
| -------------------- | ---------------- | -------------------------------------------------------------------- |
| `tracing-appender`   | `^0.2`           | Daily-rolling file appender + `non_blocking` writer / `WorkerGuard`. |
| `tracing-subscriber` | (present, `0.3`) | Add `json`, `registry`, `fmt` layering for the dual-sink JSON layer. |

`tracing` (`0.1`) and `tracing-subscriber` (`0.3`, `env-filter`) are already dependencies; this ADR adds
`tracing-appender` and the `json` feature. (Dependency edits are made by the implementing chokepoint task, not by this
documentation change.)

## Consequences

### Positive

- One taxonomy, level policy, and field vocabulary means every chokepoint task targets the same shape; logs filter and
  correlate across navigation, UI, HTTP, sync, and SQL by target, level, `character_id`, and `route`.
- The JSON file is a complete, machine-parseable record at `TRACE` for `pod` plus full SQL via `sqlx::query`, while the
  console stays a readable `info`-level skeleton — diagnosis and day-to-day watching are served by different sinks
  without trade-off.
- Reusing sqlx's native `sqlx::query` channel means zero changes to 379 call sites, no drift from the real SQL, and full
  statement/elapsed/rows for free.
- Holding the `WorkerGuard` for the program lifetime guarantees logs flush on exit, so post-mortem diagnosis keeps the
  tail of the run.

### Negative

- The dual-sink registry with per-target floors is more configuration than a single `fmt().init()`; the filter
  directive order (`sqlx=info` before `sqlx::query=debug`) is load-bearing and easy to get wrong.
- A daily-rolling JSON file at `TRACE` for `pod` (including the `pod::ui` message firehose and every query) grows; log
  retention/rotation cleanup beyond daily rolling is left to future work.
- Discipline is required: fields must be emitted as structured key–values, not interpolated into the message, or the
  JSON sink loses its indexability. This is a convention the code must uphold, not something the type system enforces.

## Future Work

- Log retention / pruning of old daily files beyond the rolling appender's date-stamping.
- `tracing` **spans** (e.g. a span per sync job or per HTTP request) to nest events and propagate `character_id` /
  `elapsed_ms` automatically, rather than attaching them per event. This ADR establishes flat events first; spans are a
  follow-on.
- An optional in-app log viewer / level control surfaced in settings.

## References

- ADR-0007 — User-Configurable Storage Paths (`0007-user-configurable-storage-paths.md`). The log directory is a
  resolved, overridable setting (`resolved_log_dir`); the file sink in §4 is rooted at that resolved path.
- ADR-0002 — Sync/Render Separation and Aggregation Chaining (`0002-sync-render-separation.md`). The sync vs UI boundary
  mirrored by the `pod::sync` vs `pod::ui` namespaces, and the dependency-chain triggers that `pod::sync`
  instrumentation reports on.
- `docs/process/writing-adrs.md` — ADR format and status lifecycle this record follows.
- Logging & observability spec (gest artifact `zznmlrxx`) — the rework this ADR is the convention for.
