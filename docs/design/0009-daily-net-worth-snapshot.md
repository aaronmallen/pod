---
id: "0009"
title: Daily Net-Worth Snapshot
status: active
tags: [aggregation, app-owned, architecture, database, sync, wallet]
created: 2026-06-06
---

# ADR-0009: Daily Net-Worth Snapshot

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The Wallet net-worth graph (spec `kvqtllux`, S3) needs up to 365 daily points per character and a combined
all-characters series. A SQL view cannot manufacture history, so `character_net_worth_snapshot` (migration `0029`) is
an **app-owned** (ADR-0003), **app-maintained** table written one row per `(character_id, date)` by a daily
**derive pass** — not an ESI mirror. This ADR settles how that pass fires and what it writes:

1. **One global pass, not per-subject.** The snapshot writer is a single, character-independent
   `JobKind::NetWorthSnapshot` that computes **every owned character's** current net worth in one run, reading the
   already-synced `character_financials` view (migration `0041`). It is **not** fanned out per character — `applies_to`
   returns `false` for all subjects, so per-subject discovery never enrolls it; the engine enrolls it once on its own
   global lane.
2. **No ESI call, so no grant.** The pass reads DB data only and makes zero ESI requests, so `required_scope()` is
   empty. That empty scope is exactly the existing "public job" convention (ADR-0005 / `public_for_subject`), reused
   rather than a parallel auth path: a job with no required scope is dispatched with `grant: None`.
3. **UTC day boundary, last-of-day wins.** `date` is the current **UTC** day as an ISO `YYYY-MM-DD` string. The upsert
   is **idempotent on `(character_id, date)`**: re-running the same UTC day overwrites that day with the latest
   reading. So the daily interval and the engine's first-cycle-after-sync trigger converge on one row per character per
   day.
4. **Forward-only current value; liquid-only history is backfilled.** Each day's row carries the current-value reading
   from the view: `liquid` and `net_worth` are always written (`liquid` defaults to `0.0` when only assets/escrow are
   synced); `asset_value`/`escrow` pass through as `NULL` when their inputs are not yet synced. Pre-first-run history
   is **liquid-only backfill** (walk the wallet journal, last-balance-of-day with carry-forward) where
   `asset_value`/`escrow` are `NULL` and `net_worth = liquid`. We never fabricate past holdings or historical prices.

No DDL is introduced here — the table (`0029`) and the view (`0041`) already exist; this ADR records the writer's
trigger model and figure semantics.

## Context

ADR-0003 establishes the project's preferred pattern for live financial scalars: derive them in a read-only SQL view
(`character_state.wallet_balance`, total SP), never as a cached column. S3's `character_financials` view (migration
`0041`) extends that to the full composite — liquid / asset value / escrow / net worth — read in one indexed query.
That covers *current* value. It cannot cover *history*: a view has no memory, and Pod cannot reconstruct what a
character's assets or the market were worth on a past day. The net-worth **graph** needs a year of daily points, so S3
Phase 3 calls for one genuinely persisted series — the only place in the financial foundation where data is stored
rather than derived.

That persisted series raises questions the spec flagged as open and left for this sign-off:

- **What fires the writer, and how often?** The series must populate without the user opening anything, exactly once
  per day, and also "immediately" after a fresh sync so the chart is not empty on first use.
- **Where does it live in the write model?** ADR-0003 defines app-owned data as written *directly by the feature that
  owns it*, "never read or written by the sync engine." A daily background pass has no feature event to hang off; the
  natural cadence host is the sync engine's scheduler. This is a genuine tension with ADR-0003's wording that must be
  reconciled, not ignored.
- **What day boundary, and what happens on multiple runs in a day?** (S3 Open Question: "UTC vs local day boundary …
  multiple runs per day keep last-of-day.")
- **What is written for days/figures we have no data for?** (S3 Open Question: the forward-fill tradeoff — net worth
  on backfilled liquid-only days.)

The global-lane *mechanism* (how a single character-independent public job is enrolled and dispatched once per cycle,
and how an app-owned derive pass is triggered on a cadence without being an ESI sync job) is decided by its own
sign-off task (`oqqzvumo`) and is the chokepoint this snapshot routes through. This ADR depends on that lane existing
and decides only the **snapshot-specific** policy: which characters, which figures, which day boundary, and the
backfill semantics.

## Decision

### 1. A single global derive pass over `character_financials`, reconciled with ADR-0003

`JobKind::NetWorthSnapshot` is a global, app-owned job. One run iterates **owned** characters — those with a
credential row, the same ownership gate the engine's per-subject discovery uses (`owned-character-vs-character-model`)
— and for each reads `character_financials::get` and upserts today's `character_net_worth_snapshot` row. It is enrolled
once (not per character) because `applies_to` returns `false` for every `Subject`, so `for_subject` /
`public_for_subject` never pick it up; the engine's global lane is its only enrollment point.

**Why owned-only.** A non-owned character has no authenticated wallet/asset/order data, so the view's figures are all
`NULL` and there is nothing of value to record. Gating on ownership matches the roster/sync convention and keeps the
series to the characters the user actually holds.

**Reconciliation with ADR-0003.** ADR-0003 says app-owned data is written by the owning feature and "never read or
written by the sync engine." The snapshot is app-owned, but its writer is a *derive pass hosted on the sync engine's
scheduler*, which is a narrower thing than "an ESI sync job":

- It makes **no ESI call** and needs **no grant** (`required_scope()` is empty). It is not fetching from ESI and
  mirroring a response — the thing ADR-0003 means by "the sync engine." It only reads already-synced DB rows through a
  view and upserts a derived figure. ADR-0002's sync/render separation is preserved: no ESI traffic originates here,
  and render still reads the DB only.
- The sync engine is used purely as a **cadence host** — the one component already running a periodic loop with the
  right cross-character scope and the first-cycle-after-sync signal. Building a second timer elsewhere to fire a pass
  that depends on freshly-synced data would duplicate the scheduler and lose the natural "right after a sync" trigger.
- The financial figures it reads are the proper output of derive views (ADR-0003); persisting the *history* of those
  derived figures is the genuine exception ADR-0003's "derive, never store" guidance allows, justified because a view
  cannot reconstruct past days.

So this is recorded as an **intentional, scoped extension of ADR-0003**: app-owned data may be maintained by a global,
grant-free *derive pass* on the sync scheduler, provided the pass makes no ESI call and reads only already-synced data.
It remains app-owned (application-allocated `id`, FK to `characters` with cascade, excluded from sync discovery); only
its *writer's host* is the scheduler rather than a UI event.

### 2. Public-job convention reused for the grant-free lane

Because the pass reads DB data and calls no ESI endpoint, `JobKind::NetWorthSnapshot::required_scope()` returns the
empty slice. The engine already treats an empty required scope as "public": `run_job` constructs the `JobCtx` with
`grant: None` when the scope is empty and skips the token lookup entirely. The snapshot lane therefore needs **no new
auth path** — it rides the same public-vs-grant branch the existing no-scope jobs (`CharacterProfile`,
`CharacterAbyssals`) use. The job body never touches `ctx.grant` or `ctx.esi`.

### 3. UTC day boundary; idempotent last-of-day upsert

`date` is the current day in **UTC**, formatted `YYYY-MM-DD`. UTC is chosen over local time because:

- EVE's own day boundary and the journal/order timestamps Pod stores are UTC, so a UTC snapshot day lines up with the
  data it aggregates and with the carry-forward liquid backfill (which samples the last journal balance of each UTC
  day).
- The ISO string makes lexical ordering double as chronological ordering, which the timeframe range slices
  (`WHERE date >= ?`, 1W/1M/3M/6M/1Y) and the combined-series `GROUP BY date` rely on. A local boundary would make the
  same calendar day differ per machine and break a shared, portable history.

The writer upserts on the `UNIQUE(character_id, date)` index (`ON CONFLICT … DO UPDATE`), so **re-running the same UTC
day overwrites that day** with the latest reading — last-of-day wins. This makes the two triggers safe to combine: the
daily `interval()` (24h) gives the steady-state once-per-day cadence, and the engine additionally force-runs the pass
on the **first cycle after a fresh sync** so the chart populates immediately; if both fire on the same UTC day, the
second simply overwrites the first.

### 4. Forward-only figures; liquid-only backfill with `net_worth = liquid`

The snapshot table types `liquid` and `net_worth` as `NOT NULL` and `asset_value` / `escrow` as nullable, encoding the
history model:

- **Forward-only (today's row, written by this pass).** From the view: `net_worth` is required, so a character whose
  view `net_worth` is `NULL` (fully unsynced) is **skipped** — there is nothing to record yet, and the next pass picks
  it up once data lands. `liquid` is written as the view's liquid or `0.0` when only assets/escrow are synced.
  `asset_value` and `escrow` are written as the view returns them (`NULL` when their inputs are not yet synced), never
  coerced to `0`, so a partially-synced day is honest about which terms are real.
- **Liquid-only backfill (pre-first-run history).** On first run the series is backfilled from the accumulating wallet
  journal (ADR-0003: the journal grows past ESI's ~30-day window) — last-running-balance-of-each-UTC-day, carrying
  forward the prior day's balance across gap days, up to ~1 year. Those days carry `asset_value = NULL`,
  `escrow = NULL`, and `net_worth = liquid`, because past holdings and historical prices are unknown and are never
  fabricated. (This backfill is its own task within S3; this ADR fixes its semantics, which the forward-only writer's
  `net_worth = liquid` rule on a liquid-only day mirrors.)

**Consequence to record (S3 forward-fill Open Question, assumption taken):** because backfilled days set
`net_worth = liquid` while forward days add asset/escrow, the series visibly "steps up" on the first-run day when
assets and escrow first appear. The spec leaned toward this forward-fill over leaving pre-history net worth blank or
back-projecting fake asset values; this ADR ratifies that choice. Whether the graph visually marks the
backfilled-vs-forward boundary is a UI decision left to the graph spec, not settled here.

## Affected Areas

- `src/sync/job.rs` — new `JobKind::NetWorthSnapshot` variant: added to `ALL`, gated on `Feature::Wallet` (the
  snapshot derives the same wallet/asset/escrow figures the Wallet feature surfaces, so Wallet off accrues no
  history), `applies_to` → `false` for all subjects (global, never per-subject), `interval()` → 24h, empty
  `required_scope()` (grant-free public lane), and wired into `run()`'s dispatch.
- `src/sync/jobs.rs` + `src/sync/jobs/net_worth_snapshot.rs` — the new derive-pass job body: iterate owned
  (credentialed) characters, read `character_financials`, upsert `character_net_worth_snapshot` for today's UTC date.
- `src/sync/engine.rs` (owned by the global-lane task, **not** edited here) — enrolls and force-after-sync triggers the
  single global instance; reuses the empty-scope → `grant: None` branch.
- `character_net_worth_snapshot` (migration `0029`) and `character_financials` (migration `0041`) — the store this pass
  reads and writes; unchanged by this ADR.
- The Wallet graph / combined-series view (`character_net_worth_snapshot_combined`) — downstream reader of the series
  this pass maintains.

## Consequences

### Positive

- The net-worth graph populates automatically — once per UTC day plus immediately after a fresh sync — with no user
  action and no render-time computation (ADR-0002 preserved: render reads the DB only).
- One global pass values every owned character in a single run, not N per-character jobs, so cost is independent of
  character count and there is exactly one writer of the series.
- Reusing the empty-scope public-job convention means no new auth path: the grant-free lane is the existing
  public-vs-grant branch, and the job body never touches a grant or ESI.
- Idempotent UTC-day upsert makes the daily and first-cycle-after-sync triggers freely combinable and re-runnable;
  there is no double-row hazard.
- The forward-only / liquid-backfill split keeps the series honest — no fabricated historical asset or price values —
  while still showing a full liquid history from day one.

### Negative

- The snapshot is **app-owned data written by the sync scheduler**, a deliberate narrowing of ADR-0003's "never
  written by the sync engine." Mitigated by the strict boundary recorded here (no ESI call, no grant, reads only
  already-synced data) and by the empty-scope marker that keeps it off every credentialed/ESI path.
- The series **"steps up"** on the first-run day as asset/escrow appear over a backfilled liquid-only history; this is
  the accepted forward-fill tradeoff, visible in the graph until (optionally) the graph spec marks the boundary.
- A character synced for the first time mid-day is **skipped until it has any figure**, so its series starts on the
  first pass after data lands rather than at account creation — acceptable, since there is no valuation to record
  before then.
- Persisting a derived figure's history is an exception to ADR-0003's "derive, never store"; justified because a view
  cannot reconstruct past days, but it means the stored series and the live view could disagree for a past day if the
  underlying journal is later corrected (the stored history is a point-in-time reading, by design).

## Open Questions

- **Backfill task ownership.** The liquid-only journal backfill is a sibling S3 task; this ADR fixes its
  `net_worth = liquid`, last-of-UTC-day, carry-forward semantics but does not implement it. The forward-only writer
  here applies the same `net_worth = liquid` rule to a liquid-only *current* day.
- **Graph boundary marking.** Whether the graph visually distinguishes backfilled-liquid-only history from
  forward-only full data is left to the graph/Wallet UI spec.
- **First-cycle-after-sync trigger wiring.** The exact engine signal that force-runs the pass right after a sync is
  owned by the global-lane task (`oqqzvumo`) and `src/sync/engine.rs`; this ADR only requires that such a trigger
  exists and is idempotent with the daily cadence.

## References

- ADR-0002 — Sync/Render Separation (`0002-sync-render-separation.md`). No ESI traffic originates from this pass and
  render still reads the DB only; the derive pass does not violate the separation.
- ADR-0003 — Canonical Data Model (`0003-canonical-data-model.md`). The snapshot is app-owned (application-allocated id,
  FK with cascade, excluded from sync discovery), and this ADR records the scoped extension that lets a grant-free
  derive pass on the scheduler maintain it; it also extends the derive-via-view pattern that `character_financials`
  uses for current value, persisting the *history* of those figures as the exception this ADR justifies.
- ADR-0005 — EVE SSO Authentication and Deeplink Transport (`0005-eve-sso-authentication-and-deeplink-transport.md`).
  The public-vs-grant convention (empty `required_scope()` ⇒ no credential needed) this lane reuses instead of a
  parallel auth path.
- Spec — "S3: Market Prices & Financial Aggregation" (gest artifact `kvqtllux`): Phase 3 history model and the
  forward-fill / day-boundary / cadence Open Questions this ADR resolves.
- Global-lane sign-off task `oqqzvumo` — the character-independent global-job lane and derive-pass cadence mechanism
  this snapshot routes through.
- Project rule — owned-vs-character (`owned-character-vs-character-model`): owned = has a credential row; this pass
  gates on it.
