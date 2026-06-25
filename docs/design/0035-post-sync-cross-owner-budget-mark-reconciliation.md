---
id: "0035"
title: Post-Sync Cross-Owner Budget Mark Reconciliation
status: active
tags: [data-model, budget, sync, persistence]
created: 2026-06-23
---

# ADR-0035: Post-Sync Cross-Owner Budget Mark Reconciliation

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

A budget mark placed on one owner's copy of a multi-owner financial event must propagate to the other owners' copies
even when those copies sync hours later. ADR-0028 made each per-entry assignment owner-aware so a character copy and a
corporation copy are distinct rows; it left open the temporal gap that the mark-time cross-owner cascade only fills
copies that already exist in memory. We close that gap with a write-side, post-sync reconciliation job
(`JobKind::BudgetAssignmentReconcile`) that materializes the missing per-owner assignment copies after every wallet
sync — fill-only, override-respecting, and guarded against resurrecting a deliberately removed mark. We explicitly
reject read-side fallback and a canonical-event table.

## Context

A corp-on-behalf market sale surfaces in two owned wallets that sync at very different speeds: the character wallet
(fast) and the corporation wallet journal (slow — ESI caches the corp journal server-side for ~1hr). The mirrored legs
share EVE identifiers — the same `transaction_id` for the market rows, the same journal id/`journal_ref_id`/`context_id`
for the journal, tax, and broker-fee legs.

ADR-0028 established that each assignment is one materialized row per (scope, owner, entry_kind, entry_id), and the
wallet chip cascades a mark to an event's twins under both owners at mark-time. But that cascade
(`src/features/wallet.rs:3609-3729`) scans only the in-memory loaded ledger (`state.market`/`state.journal`, paginated
to 50 rows and scope-filtered) and runs once, when the user marks. When the pilot marks the fast-arriving character
legs, the corp rows have not synced, so there is nothing to cascade onto — and nothing re-runs the cascade when the corp
journal arrives later. The corp legs land unmarked: visible per-row as unmarked chips and counted by the Uncategorized
journal-leg loop (`src/features/budget.rs:1555-1573`). Monthly totals are usually still correct because
`activity_by_month` de-dups market rows by `transaction_id`; the damage is the per-row chips and the journal-leg
uncategorized count.

This is the same class of cross-owner integrity problem ADR-0028 and migrations 0099/0102 were created to enforce — now
exposed along the time axis rather than the identity axis.

## Decision

Re-run the cross-owner cascade as a post-sync reconciliation pass instead of only at mark-time, healing toward the
ADR-0028 invariant (exactly one materialized assignment row per (owner, entry_id)) rather than working around it.

1. A new global, idempotent sync job `JobKind::BudgetAssignmentReconcile`, modeled on `KillmailReconcile`, chained from
   the `on_success_triggers` of `CharacterWallet | CorporationWallet` so it runs after every wallet sync — including the
   late corp sync.
2. Set-based DB reconciliation, not a lift of the in-memory cascade. The in-memory `budget_cascade_targets` is paginated
   and scope-filtered and would miss legs that exist locally but are off the loaded page or in a non-active scope.
   Reconciliation is a single set-based query over the All scope using the same linkage the cascade encodes: market
   mirror via shared `transaction_id` across owners; journal twins via `journal_ref_id`; transfer/tax/broker legs via
   `context_id == transaction_id` with `ref_type IN MARKET_FEE_REF_TYPES`; and market_transaction journal legs via
   `context_id`.
3. Fill-only and override-respecting. For each event group where at least one owner copy is assigned, the pass inserts
   the same category for any sibling owner that holds a real wallet row for the leg (`owner_holds_entry`, inherently
   satisfied because the freshly-synced row is what triggered reconciliation) and has no assignment of its own. An owner
   that already holds any assignment is never touched, so a deliberately different corp-side mark is preserved. Writes go
   through the existing `upsert_entry_assignment`, whose `ON CONFLICT` on the owner-aware unique index makes the pass
   idempotent and interrupt-safe.
4. Resurrection guard. A copy is only propagated to a sibling when the source mark's `updated_at` is newer than the
   sibling leg's arrival / last unassignment, so a deliberate unassignment wins (reusing migration 0102's "newest
   updated_at wins" rule). Unassignment is made authoritative at the write side: chip- and bulk-unassign delete the mark
   DB-side across all owners holding the event id, not just the in-memory cascade targets, so the reconciler has nothing
   to resurrect.
5. GC on the same hook. `prune_orphan_entry_assignments` (previously uncalled) runs in the same job, so a reconciled
   copy whose wallet row later disappears is collected on the next pass — keeping the table self-healing alongside the
   materialize step.
6. One-time backfill. A numbered migration mirror-fills the copies the broken mark-time cascade failed to write for
   events already split, using the same fill-only / override-respecting logic in SQL with `NOT EXISTS` guards
   (idempotent, re-runnable). It only fills missing copies; it does not collapse or re-key (migration 0102 already healed
   the historical aliasing).

Conflict policy: automation propagates intent into gaps but never overrides an explicit human decision on either copy.
Differing char/corp marks stay independent at runtime; "newest updated_at wins" is reserved for the one-time backfill,
not the steady-state job.

## Affected Areas

- `src/sync/job.rs` — new `JobKind` member, ALL/global registration, dispatch arm, interval, and the
  `on_success_triggers` wallet chaining.
- `src/sync/jobs/budget_assignment_reconcile.rs` — new job (mirrors `killmail_reconcile.rs`).
- `src/store/repo/budget.rs` — new set-based reconciliation query; new cross-owner delete-by-event query;
  `prune_orphan_entry_assignments` promoted from dead code to a live caller.
- `src/features/wallet.rs` — chip- and bulk-unassign route through the DB-side cross-owner delete.
- `migrations/` — one-time backfill migration.

## Consequences

### Positive

- A mark survives the sync-time skew: the pilot marks an event once and its slow-arriving legs are correctly categorized
  when they land, with no re-marking and no stray Uncategorized entries.
- The fix reinforces the ADR-0028 materialized owner-keyed model instead of adding a parallel resolution path; every
  read surface (chips, the Uncategorized count, `activity_by_month`) keeps resolving strictly by (owner, entry_id) with
  no read-path changes.
- Idempotent and interrupt-safe: re-running the job (or re-applying the migration) is a no-op.
- The delete-skew mirror bug (unassigning one owner while the sibling is unsynced leaving an orphan) is closed in the
  same change.

### Negative

- Reconciliation materializes real rows, so `budget_entry_assignments` grows roughly in proportion to mirrored events
  (vs. a read-side fallback that writes nothing). The owner-aware unique index bounds duplication, and orphan GC trims
  rows whose wallet legs disappear.
- A second source of assignment writes (sync job + backfill) now exists alongside the UI cascade; the two must keep the
  same linkage rules in step. The job and the backfill share their fill SQL to avoid divergence.

## Future Work

- A first-class canonical logical-event identity (so non-wallet sources — contracts, industry jobs, manual adjustments —
  can share one budget envelope) is deferred. It still needs this post-sync hook to attach late legs, so it is strictly
  additive to this decision, not an alternative.

## References

- Supersedes nothing; extends ADR-0028 along the time axis.
- Spec: gest artifact `mqzpprvw` (Cross-Owner Budget Mark Reconciliation).
- Migrations 0099 (owner-aware identity) and 0102 (one-time assignment repair).
- Root cause: `src/features/wallet.rs:3609-3729`; Uncategorized journal-leg loop `src/features/budget.rs:1555-1573`;
  assignment repo `src/store/repo/budget.rs:172-235,593-656`.
</content>

</invoke>
