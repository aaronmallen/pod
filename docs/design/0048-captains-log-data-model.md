---
id: "0048"
title: Captain's Log Data Model and Rollup on Read
status: active
tags: [data-model, captains-log, persistence, aggregation]
created: 2026-07-13
---

# ADR-0048: Captain's Log data model and rollup on read

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The Captain's Log is a per-day journal of a pilot's activity across the whole roster: user-authored narrative and
prompt answers, kill and loss debriefs, a record of skills that finished training, and notes on calendar events. This
ADR settles two decisions the feature rests on. First, the daily activity rollup (ISK earned and spent, net-worth
delta, kills and losses, skills completed, industry jobs delivered, calendar events) is recomputed on read from the
existing synced tables and is never persisted, so the log cannot drift from the wallet, killmail, and snapshot data it
summarizes (the single-source-of-truth stance ADR-0044 took for the budget). Second, only four small tables are added,
each of which holds user-authored content or a captured event that no existing table records: `captains_log`,
`killmail_report`, `skill_completion`, and `calendar_event_note`. Skill completions are the one genuinely transient
signal in the domain, so they are captured the moment they are detected and then verified against the next skillqueue
sync. No row in any of these tables is created without explicit user input, except `skill_completion`, which records a
detected event rather than typed content.

## Context

The Captain's Log needs persistence for two different kinds of data, and the split between them is the whole design.

Most of what a daily log shows already exists in synced tables. Wallet money-flow is in `character_wallet_journal`,
kills and losses are in `character_killmails`, net worth is in `character_net_worth_snapshot` (ADR-0009), industry jobs
are in `character_industry_jobs`, and calendar events are in `character_calendar`. None of this needs a new table. A
day's rollup is a query over these sources, scoped to the roster and grouped by calendar day.

Two things are missing. There is nowhere to store what the user writes (the day's narrative, the prompt answers, a
debrief on a specific loss, a note on a calendar event), and there is no record that a skill finished training. A
completed skill leaves no row anywhere: it disappears from the skillqueue and simply raises the trained level on the
character sheet. The existing skill-completion detector (in `src/features/shell/notifications.rs`) notices the
transition and fires a notification, but nothing durable is written, so the log has no way to answer "which skills
finished on this day."

The tempting shortcut for the rollup is to snapshot it: compute each day's figures once and store the result. ADR-0044
argued against exactly this shape for the budget. Reconstructing a figure into its own persisted copy means the copy
can disagree with the source it was derived from, and every later correction to the source (a re-synced journal, a
re-valued killmail) silently diverges from the frozen snapshot. The budget rebuild made the wallet journal the single
source of truth and deleted the parallel state layered on top of it. The Captain's Log rollup is the same situation:
its inputs are already persisted and already corrected by their own sync jobs, so a second stored copy of their daily
aggregate would only be another thing to keep in agreement.

ADR-0009 is the deliberate counter-example and marks the boundary. The daily net-worth snapshot is persisted precisely
because a SQL view cannot reconstruct a past day's asset valuation or historical market price. That is genuinely
non-derivable history, so it is stored once and never recomputed. The Captain's Log rollup is the opposite: every input
it reads is already on disk, so there is nothing to reconstruct and no reason to store the aggregate.

## Decision

### Four tables, each for content or an event nothing else records

The feature adds four tables and no others. Everything else the rollup needs is read from existing synced tables.

1. `captains_log`: one row per calendar day the user has authored content for, account-scoped. It holds the day's
   narrative and the per-prompt answers (goal, remember, blocked, build, skill, combat, next, research), plus
   `created_at` and `updated_at`. A day with no user content has no row.
2. `killmail_report`: one row per `(character_id, killmail_id)`, modeled on the mail-draft table. It holds the debrief
   the user writes about a specific kill or loss: an outcome classification (clean, costly, learning), what happened
   (required), what they would do differently, and a takeaway. Upsert is idempotent per `(character_id, killmail_id)`.
3. `skill_completion`: the history of detected skill completions, keyed by `(character_id, skill_id, level)`, with the
   completion time and a verified flag. This is the one table written from a captured event rather than typed content
   (see below).
4. `calendar_event_note`: a user's note text keyed by calendar event id, so a note survives even though the underlying
   calendar entry is a live-queue overlay that can vanish on the next sync.

### Account scoping via the owned_characters view

The log is per account, not per character. A day is a single log across the whole roster, so `captains_log` has one row
per calendar day rather than one per `(character, day)`. The rollup aggregates over the `owned_characters` view
(ADR-defined roster membership: a character with a credential row), grouping money-flow, kills, skills, industry, and
calendar rows by calendar day with the same `substr(date, 1, 10)` day-bucketing pattern the finance queries already
use. The user reads and writes one journal for the account; the underlying per-character source rows are folded into
that single per-day view at read time.

### The daily rollup is recomputed on read and never persisted

A rollup service computes a day's figures on demand from the existing tables:

- ISK earned, spent, and net from `character_wallet_journal` split by sign.
- Net-worth delta (absolute and percentage) by differencing consecutive `character_net_worth_snapshot` days. A gap day
  with no snapshot yields no delta and is not interpolated.
- Kills and losses (count, ISK, and a per-engagement list) from `character_killmails`.
- Skills completed from the new `skill_completion` table.
- Industry jobs delivered that day from `character_industry_jobs`.
- Calendar events for the day from `character_calendar`.

Nothing here is stored. The same service also owns the "has activity" and completeness predicates (a day with any of
the above, a day missing a required answer, a loss with no debrief) so the view and the MCP layer share one definition
rather than each re-deriving what counts as an active or incomplete day.

The rollup is not snapshotted, for the reason ADR-0044 gives: its inputs are already the single source of truth and are
already corrected by their own sync jobs, so a persisted daily aggregate could only drift from them. Persisting it would
recreate the parallel-state problem the budget rebuild deleted. The one persisted series the rollup reads,
`character_net_worth_snapshot`, is stored under ADR-0009 for the narrow reason that a past day's valuation is not
otherwise recoverable; the rollup consumes that series but adds no snapshot of its own.

### Capture-then-verify for skill completions

Skill completions are captured, not derived. When the existing detector fires, `skill_completion` gets a row
immediately, marked unverified. This is necessary because the signal is transient: once a skill finishes it leaves the
skillqueue and only raises the trained level, so if the completion is not written at detection time there is no later
query that can reconstruct which skill finished on which day.

A captured row is then reconciled against the next skillqueue sync. A completion the queue confirms is marked verified.
A completion the queue contradicts (a paused queue, a plan change, or clock skew that moved or removed the finish) is
corrected or deleted. Capture is forward-looking only: there is no backfill of completions from before the feature
shipped, because the transient signal for those days is already gone.

Calendar notes have a related property that motivates their own table. A calendar entry is a live-queue overlay from
ESI: it reflects the current calendar and can disappear on the next sync. Keying the user's note by event id in
`calendar_event_note` keeps the note even when the event it annotates is no longer in the queue.

### No row without user input

Except for `skill_completion` (which records a detected event), none of these tables gets a row unless the user typed
something. A day with activity but no writing has no `captains_log` row, a kill with no debrief has no `killmail_report`
row, and a calendar event with no note has no `calendar_event_note` row. The rollup still reports the day's activity
from the synced tables; the authored tables stay empty until the user contributes. This keeps the authored data honest
about what the user actually wrote and keeps the "incomplete day" predicate meaningful (a day is incomplete because a
prompt is unanswered, not because a row was auto-created empty).

## Affected Areas

- `src/store/model/` and `src/store/repo/` gain models and repositories for the four tables, following the budget and
  mail-draft repo patterns.
- A new rollup service aggregates over the `owned_characters` view for a given date and owns the shared has-activity and
  completeness predicates.
- The skill-completion detector (`src/features/shell/notifications.rs`) gains a write to `skill_completion` at detection
  time, and the skillqueue sync path gains the reconcile pass that verifies or corrects captured rows.
- New sequential migrations add the four tables. No existing table is altered; the rollup reads
  `character_wallet_journal`, `character_killmails`, `character_net_worth_snapshot`, `character_industry_jobs`, and
  `character_calendar` unchanged.

## Consequences

### Positive

- The daily rollup cannot drift from the wallet, killmail, snapshot, industry, and calendar data it summarizes, because
  it is recomputed from them on every read rather than copied into a snapshot.
- The persistence surface is small: four tables, each justified by content or an event nothing else records. No figure
  that an existing table already holds is duplicated.
- Skill completions become queryable per day without a backfill, and the capture-then-verify pass keeps a transient
  signal accurate against the authoritative skillqueue.
- The has-activity and completeness rules live in one service, so the view and the MCP surface agree on what an active
  or incomplete day is by construction.

### Negative

- The rollup does real aggregate queries on every read rather than a single indexed lookup, so a very long date range is
  more expensive than reading a snapshot table would be. This is the deliberate trade the single-source-of-truth stance
  accepts, and the date-bucketed queries match the pattern the finance views already use at acceptable cost.
- Skill completions and calendar notes exist only from the feature forward. Days before the feature shipped show no
  completed skills and carry no event notes, because the transient signal for those days is unrecoverable.
- A day's historical figures can change after the fact if an underlying source is re-synced or re-valued (a corrected
  journal, a re-valued killmail). This is intended: the log reflects the current best reading of its sources rather than
  a frozen point-in-time copy.

## References

- ADR-0044 (Budget = Journal, single source of truth): the stance this ADR reuses for the rollup. A derived figure is
  computed from its authoritative source on read, not persisted into a parallel copy that can drift.
- ADR-0009 (Daily Net-Worth Snapshot): the deliberate counter-case. That series is persisted because a past day's
  valuation is not otherwise recoverable; the Captain's Log rollup persists nothing because all of its inputs already
  are.
- Spec: gest artifact `woxuqrnn` (Captain's Log data and aggregation), sub-spec 1 of parent `ylrktqtl`.
- Patterns: `src/store/repo/budget.rs`, `src/store/repo/finance.rs` (date-bucketed aggregation), the mail-draft table
  (killmail-report shape), and the `owned_characters` roster view.
