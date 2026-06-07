---
id: "0002"
title: Sync/Render Separation and Aggregation Chaining
status: active
tags: [aggregation, architecture, sync, ui, wallet]
created: 2026-06-06
---

# ADR-0002: Sync/Render Separation and Aggregation Chaining

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The background sync engine and the UI render layer are fully decoupled. Sync writes complete records to the database;
render reads from the database and displays what it finds. The database is the only shared *data* contract between them,
and every record in it is guaranteed to be complete and immediately renderable. The UI may additionally send sync a
narrow, data-free **control** signal — enroll, withdraw, or nudge a subject — over a separate channel, but it never
sends data and never steers execution.

Within sync, derived figures follow a **dependency-ordered aggregation chain**: an aggregation job (one that computes a
figure from already-synced data rather than fetching from ESI) runs immediately after its upstream data jobs complete,
with its periodic interval demoted to a fallback cadence.

## Context

Pod continuously fetches character data from ESI across multiple authenticated characters. ESI responses are often
spread across several endpoints — an asset entry may require separate calls for item type info, icon URLs, and (for
abyssal modules) mutated stats and MutaMarket pricing. Each endpoint carries its own cache window and effective rate
limit.

Two naive approaches both fail:

**Fetching inside the render layer** couples UI responsiveness to network latency. Views block waiting on data,
partial state becomes visible, and error handling bleeds into display logic.

**Persisting incrementally** (writing each ESI response as it arrives) means the database can contain records that
are only partially assembled. The render layer must then handle missing fields everywhere, indefinitely — there is no
structural guarantee that a record is safe to display.

A separate requirement adds tension with a strict no-coupling reading: when a user adds a character or corporation,
sync must be **invokable immediately** and then continue autonomously. A freshly added subject has a credential row but
no synced records yet. This has to be satisfiable without letting the UI feed data to sync or steer its execution — the
first sync may start sooner, but the UI must never be able to make sync show half-assembled data or exceed ESI limits.

## Decision

Sync and render operate in fully separate layers. The database is their only shared *data* interface; a narrow control
channel is the only other coupling between them, and it carries no data.

**The invariant: a record in the database is always complete and renderable.**

Sync assembles the full picture for a record — following all dependency chains across ESI endpoints — before writing
anything. If any required request fails, nothing is persisted for that record in that cycle and the attempt is retried
on the next cycle.

Render reads from the database only. It has no knowledge of in-flight sync state, partial fetch results, or network
errors. If a record is not in the database, it is not shown. This is expected behaviour during the first sync and is
communicated to users through sync lifecycle events, not through UI-level data-loading states.

Binary presentational assets (portraits, logos, type renders) are *data*, not an exception to "render never touches the
network": the sync engine fetches them while building an entity's complete dataset, and a row is committed only once its
images are present, so the render layer never fetches and never shows a row without its images. See
[ADR-0013](0013-committed-item-icon-set.md).

Sync announces coarse lifecycle events — job started, progress milestone, job finished, job failed — that the UI may
observe for status indicators and progress bars. These events are observe-only; the UI cannot reply on the event
channel.

### Rate limiting

Rate limiting is owned entirely by sync. Each job's effective rate is the minimum cache window across all ESI
endpoints it must call. Jobs track `next_allowed_at` per endpoint and self-throttle; they never exceed ESI rate limits
regardless of how many characters are enrolled.

### Control plane (UI → sync)

The UI MAY send the engine a narrow, data-free **control** signal over a dedicated channel (`tokio::sync::mpsc`),
separate from the event channel:

- `Enroll { subject }` — "this subject now exists; include it in the autonomous schedule."
- `Withdraw { subject }` — "this subject was removed; stop scheduling it."
- `RunNow { subject }` — a non-binding hint to mark the subject's jobs due now.

Constraints that keep this from being execution coordination:

- A command carries only a subject identifier — never endpoints, ordering, cadence, rate budget, data, or records.
- Commands are fire-and-forget and unacknowledged; the UI cannot block on or read back from them.
- The engine remains free to coalesce, defer, or ignore any command. `RunNow` on an already-running or rate-limited job
  is a no-op; the engine still self-throttles and still discards a cycle it cannot complete.
- The control plane is an **accelerator, not a dependency**: the engine MUST be able to discover subjects by polling
  `credential::all` (and `character::all` for public-only subjects), so the system stays correct if the control channel
  is never used. A control signal is treated identically to discovering a new credential row.

`RunNow` is the signal that most strains "no execution coordination," because it influences near-term timing. It is
admitted as an explicitly non-binding hint with the same status as the poll-discovered "due now" condition; the engine
owns the decision. Implementations that prefer maximum purity may omit `RunNow` and rely on `Enroll` setting
`next_run_at = now`.

The net rule: **sync receives no execution or scheduling instructions, and no data flows from the UI to sync.** The UI
may communicate *which subjects exist* and drop a non-binding run-now hint; sync schedules everything at its own
discretion.

### Identity and credentials

Character identity and OAuth credentials are stored in **separate database tables**. A credential row references a
character by id but carries its own lifecycle, so a credential can exist before its character record has been fully
synced, and refreshing a token never touches identity rows. The rationale for that split — token expiry and
refresh-token rotation — is an authentication concern; see
[ADR-0005](0005-eve-sso-authentication-and-deeplink-transport.md).

### Dependency-ordered aggregation chaining

Some sync jobs do not fetch from ESI at all — they *derive* a figure from already-synced data (an
aggregation/report job). Because sync owns all scheduling, ordering, and dependency between jobs (above), aggregation
ordering is expressed within the same engine rather than as a separate mechanism.

**An aggregation job runs immediately after its upstream data job(s) complete, in dependency order.** Its periodic
`interval()` is a **fallback** cadence, not the primary trigger. Concretely, the financial chain is **gather →
`MarketPrices` → `NetWorthSnapshot`**: a gather job (`AssetSync`, `CharacterWallet`, `CorporationWallet`) triggers
`MarketPrices` on success, and `MarketPrices` triggers `NetWorthSnapshot`.

The motivating defect (spec `yuvmkmrl`, epic `nqsmsryk`): the engine previously marked **both** `MarketPrices` and
`NetWorthSnapshot` due-now at once on a gather success, so `NetWorthSnapshot` raced the prices it needs to value assets;
and `MarketPrices` triggered nothing, so once fresh prices landed nothing recomputed the snapshot until the next gather
(~1h) or the 24h fallback. Net worth therefore reflected stale or absent prices. The issue is general: any job that
derives from synced data has a dependency order on its inputs, and firing it concurrently with — or before — those
inputs produces a figure computed against stale state.

The resolution has four parts:

1. **Aggregations fire after their inputs, in dependency order.** `JobKind::on_success_triggers()` encodes the
   dependency edges and `engine.rs::finish()` makes each triggered kind due-now (`make_due_now`) when the source job
   succeeds:
   - `AssetSync | CharacterWallet | CorporationWallet → [MarketPrices]`
   - `MarketPrices → [NetWorthSnapshot]`

   So after any gather completes, `MarketPrices` runs, and only then does `NetWorthSnapshot` run — valuing assets
   against just-refreshed prices, within one gather cycle rather than up to a day later. `CorporationWallet` is in the
   gather set so corp wallet drives the chain. The triggered kinds are the engine's **global-lane** jobs (`applies_to`
   returns `false` for every subject), so a gather on any subject converges on the single global `MarketPrices` /
   `NetWorthSnapshot` instances rather than a per-subject fan-out.

2. **The direct gather → snapshot edge is removed.** The snapshot is now reachable only *through* `MarketPrices`.
   Routing the snapshot behind prices is what guarantees it values assets against fresh prices instead of racing them.

3. **Reports update in place.** `NetWorthSnapshot` upserts on `(character_id, date)`. Because the chain can fire the
   snapshot more than once on the same UTC day (a gather-driven run and the daily fallback), the upsert makes every
   re-run overwrite today's row — last-of-day wins, no double-row hazard. See ADR-0009 for the snapshot's figure
   semantics and day-boundary policy.

4. **Intervals demote to fallback.** The `MarketPrices` (6h) and `NetWorthSnapshot` (24h) `interval()` values are
   unchanged but now act **only** as a safety net: if no gather has occurred to drive the chain, the interval still
   refreshes prices and recomputes the snapshot eventually. No aggregation depends on a long interval for freshness in
   the steady state.

This lengthens the critical path after a gather (gather → prices → snapshot) versus firing them concurrently; in
exchange the snapshot is correct. If `MarketPrices` fails, the snapshot is not re-run on that cycle and falls back to
the prior prices and the interval cadence — acceptable, since the alternative is valuing against a failed price fetch.

## Affected Areas

- `src/sync/` — all sync job definitions, the job scheduler, the `Event` output channel, and the `Command` control
  channel; owns all scheduling, ordering, concurrency, and rate-limit decisions.
- `src/sync/job.rs` — `on_success_triggers()` encodes the gather → prices → snapshot aggregation chain;
  `CorporationWallet` joins the gather set; the direct gather → `NetWorthSnapshot` edge is removed.
- `src/sync/engine.rs` — `finish()` drives `on_success_triggers` → `make_due_now`, propagating the aggregation edges.
- `src/sync/jobs/net_worth_snapshot.rs` — its `(character_id, date)` upsert is the in-place report update the chain
  relies on. The `character_financials` view's `COALESCE(price, 0)` is left as-is (honest "no market price ⇒ 0
  contribution"); the chain ensures prices are fresh before the snapshot reads them.
- `src/store/` — schema design; tables must encode completeness at the row level (no nullable required fields).
- `src/ui/` and `src/features/` — views may not call ESI or read partial state; all display data comes from store
  queries. The feature that adds a character/corporation emits `Enroll` (and optionally `RunNow`) via the sync `Handle`
  and never performs sync work itself.
- ESI client (`src/clients/esi.rs`) — used exclusively by sync, never imported by UI modules.

## Consequences

### Positive

- UI is always fast; it never waits on network I/O.
- Partial or corrupt render state is structurally impossible.
- Sync can be tested independently of any UI framework.
- Adding a new synced data type requires explicitly defining its completeness contract, which surfaces design
  questions early.
- "Invoke on add, then autonomous" is met with an instant first sync while the safety guarantees (no partial render, no
  rate-limit breach) hold — the UI cannot steer execution.
- Credential rotation and token refresh are isolated from character display state.
- The system degrades to a strict poll-only model if the control channel is removed.

### Negative

- First sync for a new character has a noticeable delay before any data appears. This is inherent to the design and
  must be communicated clearly via progress events.
- Defining completeness for complex data types (e.g. abyssal assets with multi-step dependency chains) requires
  upfront design work per job.
- A failed mid-chain ESI call discards all work for that record in that cycle, which may increase ESI request
  volume on retries.
- Two discovery sources (control channel + DB poll) must agree; enrollment must be idempotent (find-or-insert by
  `(job, subject)`) so a re-add does not duplicate or reset cadence.
- `RunNow` is a genuine (if non-binding) influence on near-term scheduling and must be reviewed carefully to avoid
  creeping into real execution coordination.

## Open Questions

- Public-only subjects (a character with no credential row) are not discoverable via `credential::all`; the poll
  fallback needs `character::all` as a second source. Confirm both discovery sources.
- Should `RunNow` ship in the first version, or should we start with `Enroll`-with-immediate-due only and add `RunNow`
  if a manual-refresh affordance is later desired?

## Future Work

- Each sync job must document its own completeness contract: which endpoints it calls, in what order, and what
  constitutes a complete record ready for persistence. This is defined per job.
- Cache invalidation strategy (when to evict stale DB records if ESI reports a resource has been deleted) is deferred.

## References

- ADR-0013 — Image Assets — Committed Item Icons and Synced Portraits/Logos (`0013-committed-item-icon-set.md`)
- ADR-0005 — EVE SSO Authentication and Deeplink Transport (`0005-eve-sso-authentication-and-deeplink-transport.md`)
- ADR-0010 — ESI Write Path / Durable Outbox (`0010-esi-write-path-outbox.md`). Extends this ADR with the one bounded
  second data direction: sync drains an app-owned outbox to perform UI-requested ESI *writes*, while render still reads
  the DB only and the control channel still carries no data.
- ADR-0009 — Daily Net-Worth Snapshot (`0009-daily-net-worth-snapshot.md`). The snapshot's global-lane enrollment,
  grant-free dispatch, UTC day boundary, and idempotent `(character_id, date)` upsert that the aggregation chain re-runs
  in place.
- ADR-0008 — Assets Data Path (`0008-assets-data-path.md`). Records the related terminal-inaccessible-structure
  exception in the asset completeness contract.
- Spec — "Dependency-ordered aggregation chaining" (gest artifact `yuvmkmrl`); Epic — "Sync data-readiness correctness"
  (gest artifact `nqsmsryk`).
- EVE ESI documentation: <https://esi.evetech.net/>
