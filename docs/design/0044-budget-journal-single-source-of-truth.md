---
id: "0044"
title: Budget = Journal (Single Source of Truth)
status: active
tags: [data-model, budget, finance, persistence]
created: 2026-06-30
---

# ADR-0044: Budget = Journal (single source of truth)

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The budget feature is envelope budgeting whose correctness depends on real wallet balance equaling the sum of category
envelopes plus Ready-to-Assign. The prior implementation reconstructed wallet money-flow from two parallel ledgers and
layered dedup and reconciliation machinery on top, which let totals drift from the wallet. This ADR makes the wallet
journal the single source of truth: category activity is the signed sum of the journal entries assigned to it,
Ready-to-Assign is the residual ESI wallet balance minus envelope-held, and the scope concept, the entry_kind
discriminator, market-twin suppression, the cross-owner assignment cascade, and the post-sync reconciler (ADR-0035) are
removed. It supersedes ADR-0035 and builds on ADR-0040.

## Context

The budget feature is envelope (YNAB-style) budgeting. Its correctness rests on one invariant: every ISK you own is
either sitting in a category envelope or in Ready-to-Assign, i.e. real wallet balance = Σ(available across categories) +
ISK-to-allocate, where per category available = carryover + assigned + activity.

The implementation violated this invariant. Instead of reading the wallet journal, it reconstructed wallet money-flow
from two parallel ledgers (wallet journal AND wallet transactions) and layered on machinery to force agreement:
internal-transfer pairing, market-twin suppression, cross-owner assignment cascades (ADR-0035), ref-type auto-maps, and
a post-sync reconciler. The reconstruction only equals reality if every piece is perfect; any dropped leg (beyond ESI
journal retention), mispaired transfer, or mis-suppressed twin desyncs the totals from the wallet. Observed failures: a
category ~2B in the red with no matching journal entries; Ready-to-Assign not matching wallet balances; a 10B
corp-to-corp transfer counted as +20B from a dropped leg.

The parallel-ledger model also grew redundant on-disk state: a scope concept (scope_kind/scope_id) that migration 0102
already forces to always-all; an entry_kind discriminator (journal vs market); a budget_scope_seeded marker; and
budget_ref_type_maps (dead code).

## Decision

Make the wallet journal the single source of truth for the budget.

1. Activity for a category is the signed sum of the journal entries assigned to it. The transaction ledger is never used
   for budget math; it remains a display/convenience lens (identify the item, initiate an assignment) whose assignment
   writes through to the underlying journal entry.
2. Ready-to-Assign is the residual real ESI wallet balance − Σ envelope-held. Because it is measured against the
   authoritative ESI balance, it agrees with the wallet by construction, and a negative category is always auditable
   down to the journal entries that produced it.
3. Ingest everything ESI returns; never suppress a journal leg. A transfer is simply two entries (−N, +N) that net in
   the real balance. This builds on ADR-0040 (per-wallet journal identity), which stopped the schema from dropping
   mirrored legs.
4. Assignment is (owner, journal id) → category. Owner (character / corporation+division) is retained because it
   identifies which wallet's journal a row belongs to; it is not scope. The scope concept is removed: the budget is
   always all wallets.
5. Removed: scope_kind/scope_id and entry_kind columns, BudgetScope, budget_scope_seeded, budget_ref_type_maps,
   market-twin suppression, the cross-owner assignment cascade, and the split-owner post-sync reconciler (superseding
   ADR-0035).
6. Kept: the rules engine, with its full match surface including item/transaction attributes (e.g. "Tritanium →
   category X" matches on type_id); only the assignment write moves to the journal entry (item-keyed rule resolves
   item → linked transaction → its journal entry). Corp-on-behalf duplicate hiding survives purely as a
   transaction-display concern; the journal single-counts it naturally.

## Consequences

### Positive

- The invariant holds by construction, so budget totals cannot silently drift from the wallet; "in the red" is
  auditable; a large amount of dedup/reconciliation machinery (and a whole sync job) is deleted; the schema sheds
  several redundant columns and tables.

### Negative

- A one-time best-effort data migration must fold existing entry_kind='market' and multi-owner cascade assignment rows
  onto their underlying journal entry (precedent: migration 0102), dropping and logging what cannot be resolved.
- Item-based rules only apply to journal entries that have a linked transaction (a bounty or transfer has no item).
- Released migrations are immutable (embedded checksum plus CRLF hazard), so this ships as new forward migrations only.

## References

- Supersedes ADR-0035 (post-sync cross-owner budget mark reconciliation): the reconciler and cross-owner cascade it
  added exist only to keep the parallel-ledger reconstruction in agreement, and are removed once the journal is
  authoritative.
- Builds on ADR-0040 (per-wallet journal identity): both legs of an internal transfer survive in the ledger, so the
  journal nets a transfer to zero with no suppression step.
- Spec: gest artifact `trrvzrsx` (budget rebuild).
- Precedent: migration 0102 (one-time idempotent budget-assignment repair).
