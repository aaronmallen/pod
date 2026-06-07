---
id: "0010"
title: ESI Write Path / Durable Outbox
status: active
tags: [architecture, esi, outbox, sync, write-path]
created: 2026-06-06
---

# ADR-0010: ESI Write Path / Durable Outbox

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod has been a strictly read-only ESI mirror: [ADR-0002](0002-sync-render-separation.md) pins exactly one data
direction (sync owns all ESI calls and writes complete rows; render reads the DB only) and forbids the UI from
importing the ESI client or sending data over the control channel. The Mail feature (S7) is the first feature that
must *mutate* EVE state — send mail, mark read, change labels, delete. This ADR ratifies the **durable Outbox** as the
**single sanctioned UI→ESI write path**, a deliberate, bounded *second* data direction that **extends ADR-0002** and
**builds on [ADR-0003](0003-canonical-data-model.md)**.

The write path is, as built:

1. An **app-owned `outbox` table** (migration `0021_create_outbox.sql`) of typed, opaque-payload pending mutations the
   UI appends to and the sync engine drains. It is app-owned in the ADR-0003 sense — app-allocated `id`, written
   directly by the feature, never an ESI mirror — used here as ADR-0003's "third write path."
2. A **kind-dispatch contract** (`src/sync/outbox.rs`): a typed `OutboxKind` discriminant, a `KindHandler` trait with
   three per-kind operations (`apply` / `execute` / `compensate`), and a `Registry` the drainer resolves handlers from.
   S2 owns this generic seam; S7-mail plugs in the four concrete handlers.
3. A **generic drainer** (`src/sync/drain.rs`) running as a single engine-owned pass each tick:
   claim → authenticate → execute → classify, with capped-exponential backoff + jitter (`MAX_ATTEMPTS = 8`), an
   at-least-once / idempotent contract, and a permanent-failure surface that hands off to per-kind compensation.

The amended invariants are stated below; the spec's Open Questions are resolved by the implementation and recorded
here as the authoritative source.

## Context

[ADR-0002](0002-sync-render-separation.md) establishes the net rule: *"sync receives no execution or scheduling
instructions, and no data flows from the UI to sync,"* and its Affected Areas pin the *"ESI client — used exclusively
by sync, never imported by UI."* [ADR-0003](0003-canonical-data-model.md) adds a second *write* path — features writing
their
own app-owned tables directly — but nothing in Pod could still change anything in EVE.

The v0.5 Mail feature breaks that: composing, sending, marking read/unread, labeling, and deleting all mutate ESI
state. The 0.5.0 prototype solved this with an in-process unbounded `mpsc` `CommandQueue`
(`tmp/scratch/0.5.0-prototype.1/src/services/command_queue.rs`) drained on each sync tick. That queue is **non-durable**
— a crash or quit between enqueue and drain silently loses the write — and has no retry/backoff, no idempotency, no
failure surface, and no reconciliation. The controller even admits *"best-effort; snooze re-expires on the next sync
cycle if this fails."*

A principled write path must: let the UI feel instant (unread counts drop the moment a mail is marked read, before any
round-trip); survive crashes and offline periods (a queued send still goes out after a restart); retry transient ESI
failures with backoff, surface permanent failures, and self-heal optimistic local state from ESI truth; and do all of
this *without* breaking ADR-0002 — render still reads the DB only, sync still solely owns the network. That last clause
is the tension: sync reading app-owned work to perform an ESI write is a genuinely new data direction and must be
ratified, not smuggled in. This ADR is that ratification, and it records the locked resolutions to the spec's Open
Questions so the implementation (already landed) has an authoritative source.

## Decision

The durable Outbox is the **only** sanctioned UI→ESI write path. The UI never imports or constructs an ESI request; it
commits an app-owned row and the sync engine performs the write. This **extends ADR-0002** (adding the bounded second
data direction) and **builds on ADR-0003** (the outbox is an app-owned table written via the third write path).

### The four amended invariants

1. **The DB remains the only shared *data* contract; no data crosses the control channel.** The UI does not hand sync
   data over the `mpsc` channel. It commits a row to the app-owned `outbox` table — exactly ADR-0003's third write path
   (a feature writing its own table directly). Sync then discovers outbox work the *same way* it already discovers
   credentials and characters: by polling the DB. The existing `Command` channel still carries no data, and **no Drain
   nudge was added** (see Open Questions) — the drainer is correct purely by polling.

2. **Render still reads the DB only and never the network.** Optimistic UI writes target the *synced mirror tables*
   (via the per-kind `KindHandler::apply`), so every row render sees is still complete and renderable per ADR-0002. An
   optimistically-flipped row is a valid, complete row that may simply be ahead of ESI truth until reconciliation. The
   outbox is the durable record that an as-yet-unconfirmed mutation is in flight; render reads it only (optionally) for
   a pending/failed indicator, never for content.

3. **Sync still solely owns ESI.** The network write happens inside the sync engine using a fresh grant from
   `token::fresh_token` ([ADR-0005](0005-eve-sso-authentication-and-deeplink-transport.md)), gated on ownership exactly
   like every privileged
   job — `drain_row` skips a row whose subject yields no grant, mirroring how `run_job` silently returns `Ok(())` when
   `fresh_token` returns `None`. The UI never imports or constructs an ESI request.

4. **Eventual consistency via reconciliation.** The optimistic local write is provisional; ESI is authoritative. A
   later Mail read-sync overwrites the mirror with truth, and a permanent outbox failure triggers a per-kind
   *compensating* local revert (`KindHandler::compensate`). State self-heals; the UI may be briefly optimistic but is
   never the source of truth.

### The outbox table (migration `0021`)

`migrations/0021_create_outbox.sql` creates the app-owned table — app-allocated `INTEGER PRIMARY KEY` (ADR-0003), not
an ESI id:

```sql
CREATE TABLE IF NOT EXISTS outbox (
  id              INTEGER PRIMARY KEY,
  subject_type    TEXT    NOT NULL,                 -- 'character' (OwnerType serialization)
  subject_id      INTEGER NOT NULL,                 -- acting character_id; ownership-gated at drain
  kind            TEXT    NOT NULL,                 -- 'mail.send' | 'mail.set_read' | 'mail.set_labels' | 'mail.delete'
  payload         TEXT    NOT NULL,                 -- JSON, opaque to S2 (Mail spec owns the schemas)
  dedupe_key      TEXT,                             -- nullable; collapses redundant mutations
  status          TEXT    NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending', 'inflight', 'done', 'failed')),
  attempts        INTEGER NOT NULL DEFAULT 0,
  next_attempt_at TEXT    NOT NULL,                 -- ISO-8601 UTC; earliest eligible drain time
  last_error      TEXT,
  created_at      TEXT    NOT NULL,
  updated_at      TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_outbox_drainable ON outbox(status, next_attempt_at);
CREATE UNIQUE INDEX IF NOT EXISTS uq_outbox_dedupe
  ON outbox(subject_id, kind, dedupe_key) WHERE dedupe_key IS NOT NULL AND status IN ('pending', 'inflight');
```

`kind` is the typed discriminant; `payload` is the kind-specific JSON the Mail spec defines. The outbox foundation is
agnostic to payload contents — it persists, schedules, and reports. The repo (`src/store/repo/outbox.rs`) exposes
`append`, `claim_due`, `mark_done`, `mark_failed`, `reschedule`, and `prune_done`; `append` is a single committed
transaction, so once it returns the write is durable.

### The kind-dispatch contract (`src/sync/outbox.rs`)

`OutboxKind` is the typed discriminant of the `kind` column (`MailSend` / `MailSetRead` / `MailSetLabels` /
`MailDelete`, serialized to `"mail.send"` etc.), with an explicit `FromStr` so a corrupt or future-versioned kind
parses to a `ParseKindError` and is *failed* rather than silently skipped. `KindHandler` is the per-kind seam, holding
**no** payload parsing or ESI calls in S2:

- `apply(db, payload)` — optimistically apply the mutation to the synced mirror at *append* time, before any drain.
- `execute(esi, grant, payload)` — perform the authenticated ESI write; returns `Result<(), clients::Error>` so the
  drainer can classify the outcome exactly like `Engine::finish` does for reads.
- `compensate(db, payload)` — revert the optimistic local change on *permanent* failure.

A `Registry` collects handlers and resolves one by kind (`resolve` distinguishes an unknown kind string from a known
kind with no registered handler; the drainer fails the row on either). S7-mail builds the registry with the four
concrete handlers; the drainer never names a concrete handler.

### The generic drainer (`src/sync/drain.rs`)

`drain` runs as a **single engine-owned pass** each tick (`Engine::maybe_drain`, every `DRAIN_INTERVAL = 5s`,
gated on the engine's global `paused_until` window). Per pass it `claim_due`s a batch (`DRAIN_BATCH = 16`) of drainable
rows (`pending`, or `inflight`/retry whose `next_attempt_at <= now`), flipping each to `inflight` in `created_at` order,
then runs the **claim → authenticate → execute → classify** skeleton per row:

1. **Authenticate** via `token::fresh_token`, ownership-gated. No usable grant ⇒ leave the row drainable and skip it (it
   drains once the subject is re-authed). A refresh *error* is treated as transient and reschedules with backoff.
2. **Execute** the ESI write through `Registry::resolve(kind)` using the engine's existing `esi::Client`. An
   unknown/unregistered kind is a *permanent* failure (it can never succeed), so the row is terminalized rather than
   re-claimed forever.
3. **Classify** the result, reusing the engine's error taxonomy:
   - **Success** (or idempotent-equivalent) ⇒ `mark_done`.
   - **Transient** (`Error::RateLimit` / `Error::ErrorLimited` / 5xx / network) ⇒ `reschedule` with a backoff delay,
     bump `attempts`, leave drainable — **until `attempts` reaches `MAX_ATTEMPTS = 8`**, at which point the next
     transient error terminalizes the row with `mark_failed`. The throttle variants honor their exact server-supplied
     `Retry-After` / error-limit reset; everything else rides the capped-exponential curve plus jitter
     (`BACKOFF_BASE_SECS = 2`, doubling, `BACKOFF_CAP_SECS = 300`, `BACKOFF_EXPONENT_CAP = 8`), mirroring
     `schedule::backoff_delay`.
   - **Permanent** (a real 4xx rejection, or an unparseable/unregistered kind) ⇒ `mark_failed`, record `last_error`,
     and hand off to the kind's `compensate` (the per-kind revert is S7-mail's to wire).
4. An `Error::ErrorLimited` during a drain carries its reset window up to `drain`'s caller (the longest seen in the
   batch) via `DrainOutcome`, so the engine folds it into the same global `paused_until` pause it uses for reads —
   outbox drains halt with all dispatch during an error-limit window.

A store error talking to the outbox aborts the *pass* (the next tick retries); a failure on an individual row never
aborts the batch.

### Idempotency, dedupe, and ordering

- **Dedupe.** `append` collapses a redundant `pending`/`inflight` mutation onto the existing row via the partial-unique
  index `uq_outbox_dedupe (subject_id, kind, dedupe_key) WHERE dedupe_key IS NOT NULL AND status IN ('pending',
  'inflight')` — re-asserting the latest desired state (`ON CONFLICT … DO UPDATE`, resetting `attempts` and
  re-arming `next_attempt_at`). A `NULL` `dedupe_key` (e.g. `mail.send`, where every send is distinct) is never
  collapsed, so those are delivered **at-least-once**.
- **Idempotency.** Re-enqueuing a mark-read or a label op is harmless: each drain *re-asserts a desired state*
  (`is_read = true`, label present/absent), not a delta, so replaying is a no-op at ESI, and a crash mid-`inflight`
  re-drains the same row safely (`claim_due` re-claims an `inflight` row whose `next_attempt_at` has passed).
- **Ordering.** `claim_due` returns rows in `created_at` order so a later op wins per `(subject_id, kind, target)`;
  cross-kind ordering is best-effort. `mail.send` is at-least-once and not dedupable — EVE assigns a fresh `mail_id`
  per send and a rare duplicate is preferable to a silently-lost one; the UI confirms via read-sync.

## Resolved Open Questions

The spec left six Open Questions; the implementation locks them as follows, recorded here as the authoritative source:

1. **Drain shape — single engine-owned pass.** A single drain pass invoked each scheduler tick
   (`Engine::maybe_drain`, `DRAIN_INTERVAL = 5s`, one `claim_due` query), **not** a per-subject
   `JobKind::OutboxDrain`. Simpler scheduling and one claim query; per-subject rate isolation was not needed because the
   batch is bounded (`DRAIN_BATCH = 16`) and the whole pass respects the global error-limit pause.

2. **Backoff curve and caps.** Capped-exponential backoff with jitter, mirroring `schedule`: base `2s`, doubling,
   ceiling `300s` (5 min), exponent capped at `8`. **`MAX_ATTEMPTS = 8`**: a row that has failed transiently this many
   times is marked `failed` on its next transient error rather than retried forever. Throttle responses
   (`Retry-After` / error-limit reset) are honored *exactly* and independently of the curve.

3. **Done-row retention — short TTL, then prune.** Successful rows are marked `done` (not deleted inline) and retained
   as a short audit/indicator window, then removed by `prune_done(before)` (delete `done` rows whose `updated_at <
   before`). `pending`, `inflight`, and `failed` rows are always retained. Retention is a TTL window, not immediate
   pruning, so the pending/failed indicator can still observe a just-completed write.

4. **Drain nudge — not shipped; poll-on-tick only.** No data-free Drain command was added to the control channel; the
   drainer relies solely on poll-on-tick discovery. This mirrors the RunNow purity stance in ADR-0002's Open Questions:
   the system is correct without the nudge, so the nudge was omitted. It may be added later as a non-binding accelerator
   with identical status to RunNow if a manual-flush affordance is wanted.

5. **Reconciliation precedence — read-sync wins.** ESI is authoritative. A successful outbox write that the optimistic
   local flip anticipated simply matches the next read-sync; a permanent failure is reconciled by the kind's
   `compensate` revert, and any residual divergence is healed when the Mail read-sync overwrites the mirror with ESI
   truth on its next pass. No separate per-row "dirty/optimistic" marker is introduced in the outbox: the `inflight`
   status plus the read-sync-wins rule are sufficient, and the per-kind compensation owns the revert.

6. **`subject_type` is character-only this iteration.** `subject_type` is `'character'` (matching `OwnerType`
   serialization). Corp-acting mail writes are out of scope for v0.5 — corp mail is not in the locked corp full-parity
   set (corp wallet-per-division and corp assets). The column admits other subject types later without a migration.

## Affected Areas

- `migrations/0021_create_outbox.sql` — the app-owned `outbox` table, its drainable index, and the partial-unique
  dedupe index.
- `src/store/model/outbox.rs` + `src/store/repo/outbox.rs` — the `Outbox` model and the persistence layer
  (`append`, `claim_due`, `mark_done`, `mark_failed`, `reschedule`, `prune_done`); agnostic to payload contents.
- `src/sync/outbox.rs` — the kind-dispatch contract: `OutboxKind`, `KindHandler`, `Registry`, `ResolveError`. S7-mail
  plugs in the four concrete handlers; the registry is empty until then, so today every claimed row is left drainable.
- `src/sync/drain.rs` — the generic drainer: `drain` / `drain_row` / `classify`, the backoff constants, and the
  `DrainOutcome` that surfaces an error-limit window to the engine.
- `src/sync/engine.rs` — `Engine::maybe_drain` runs the pass on `DRAIN_INTERVAL`, gated on the global `paused_until`,
  and owns the `Registry`.
- [ADR-0002](0002-sync-render-separation.md) — updated to back-reference this ADR as the ratified bounded second data
  direction.
- ESI write scopes (`src/clients/esi/scopes.rs`) and any re-auth flow — owned by the Mail/auth specs
  ([ADR-0005](0005-eve-sso-authentication-and-deeplink-transport.md)), **not** this ADR; mail writes additionally need
  `esi-mail.send_mail.v1` and `esi-mail.organize_mail.v1`.

## Consequences

### Positive

- The UI feels instant: the optimistic `apply` updates the mirror before any round-trip, so unread counts drop on open
  with no flicker, and the write is durable the moment `append` returns.
- Writes survive crashes and offline periods — the durable table plus poll-on-tick discovery means a queued send goes
  out after a restart with no control-channel state to lose.
- ADR-0002 is preserved in spirit: render still reads the DB only, the ESI write happens only inside sync with a fresh
  grant, and the UI never imports the ESI client.
- The contract is generic: all four mail kinds (and any future write kind) flow through the same claim/retry/reconcile
  machinery, differing only in `apply` / `execute` / `compensate`.
- Idempotency + the partial-unique dedupe index make replays and crash-mid-inflight safe, and the at-least-once
  `mail.send` path errs toward a rare duplicate over a silent loss.

### Negative

- This is an honest **second data direction** — sync reading app-owned work to perform an ESI write — a deliberate
  extension of ADR-0002. Mitigated by the strict boundaries recorded here (UI commits a row only; the write is
  sync-internal, ownership-gated, fresh-grant).
- The optimistic mirror can be briefly *ahead* of ESI truth between `apply` and the confirming read-sync; correctness
  relies on read-sync-wins reconciliation and per-kind `compensate`, which the Mail spec must wire correctly per kind.
- `mail.send` is at-least-once, so a rare duplicate send is possible (accepted: EVE assigns a fresh `mail_id` per send
  and a duplicate is preferable to a lost send).
- The 5s poll interval bounds write latency from the bottom; without the (deliberately omitted) Drain nudge, a write
  waits up to one `DRAIN_INTERVAL` plus its backoff. Acceptable for mail; a nudge can be added later if needed.
- The registry is empty until S7-mail plugs handlers in, so until then a claimed row resolves to no handler and is left
  drainable / would fail on an unregistered kind — by design, the foundation lands before its first consumer.

## References

- [ADR-0002](0002-sync-render-separation.md) — Sync/Render Separation. The data-direction invariant, the data-free
  `Command` control plane, and "ESI client never imported by UI" that this ADR **extends** with the bounded write path.
- [ADR-0003](0003-canonical-data-model.md) — Canonical Data Model. The "third write path" (features write their own
  tables
  directly) and app-allocated ids the outbox table reuses.
- [ADR-0005](0005-eve-sso-authentication-and-deeplink-transport.md) — EVE SSO Authentication and Deeplink Transport.
  The token/grant refresh-rotation the drainer's
  `fresh_token` call uses, and the open question on the requested scope set / incremental re-auth (mail write scopes).
- Spec — "S2: ESI Write Path (Outbox)" (gest artifact `utysxyrt`): the outbox table, repo, drain/retry/reconcile
  machinery, and the Open Questions this ADR resolves.
- Sibling spec — "S7: Mail" (gest artifact `kmrurkmq`): the mail mirror tables, the read-sync job, and the four
  concrete per-kind payload encoders/appliers/compensations that implement this contract; see
  [ADR-0011](0011-eager-full-body-mail-sync.md).
- Implementation — `migrations/0021_create_outbox.sql`, `src/store/model/outbox.rs`, `src/store/repo/outbox.rs`,
  `src/sync/outbox.rs`, `src/sync/drain.rs`, `src/sync/engine.rs`.
- Prototype (replaced) — `tmp/scratch/0.5.0-prototype.1/src/services/command_queue.rs` (non-durable in-memory
  `CommandQueue`) and `tmp/scratch/0.5.0-prototype.1/src/controllers/mail.rs` (the enqueue sites and "best-effort … if
  this fails" comment).
