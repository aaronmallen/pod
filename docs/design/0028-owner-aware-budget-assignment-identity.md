---
id: "0028"
title: Owner-Aware Budget Assignment Identity
status: active
tags: [data-model, budget, persistence]
created: 2026-06-20
---

# ADR-0028: Owner-Aware Budget Assignment Identity

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

A per-entry budget assignment — the envelope a user pins onto a single wallet-journal or market-transaction row — is
identified by its **owning entity** (a character or a corporation) in addition to the scope, the entry kind, and the
EVE entry id. `budget_entry_assignments` gains `owner_kind` (`character`/`corporation`) and `owner_id` columns, and the
uniqueness key becomes `(scope_kind, COALESCE(scope_id, -1), owner_kind, owner_id, entry_kind, entry_id)`. The owner is
modelled at the feature layer by a `BudgetOwner` enum (`Character(i64)` / `Corporation(i64)`), mirroring the existing
`BudgetScope`.

## Context

EVE journal `ref_id`s and transaction `transaction_id`s are unique only **per owner**, not globally. The "All" budget
scope (`scope_id` NULL) merges every owned character and corporation into one ledger, so two owners that happen to
share an id aliased onto a single assignment row — and therefore a single category. The concrete failure: a corp market
trade is mirrored into the trading character's personal wallet with the **same** `transaction_id`, present as a primary
key in both `character_wallet_transaction` and `corporation_wallet_transaction`. Keyed only by `(scope, entry_kind,
entry_id)`, the character's copy and the corp's copy resolved to the same override and could not be assigned to
different categories. This is the cross-owner root cause behind the issue #38 double-count.

The assignment resolution path (`ResolutionContext`, `override_for`, `monthly_activity`) loaded overrides into maps
keyed by a flat entry id, which carries the same collision. The chip assign/clear path likewise wrote and matched on
`(scope, entry_kind, entry_id)` only.

## Decision

Identify each assignment by its owner as well. The migration (0099) adds `owner_kind`/`owner_id`, replaces the unique
index to include them, and backfills existing rows:

- **Scoped rows** (`character`/`corporation` scope) take their owner directly from the scope.
- **All-scope rows** derive the owner by joining the ledger on the entry id: journal rows via
  `character_wallet_journal.id` then `corporation_wallet_journal.id`; market rows via the matching `transaction_id`
  columns. The character wallet is preferred on the corp-mirror collision.
- **Documented safe default:** any All-scope row that matches neither ledger is left at `owner_kind = 'character'`,
  `owner_id = 0`. The assignment row is retained (no assignment is lost), but the sentinel `0` owner matches no live
  entry, so it contributes nothing rather than mis-bucketing — a deliberately inert fallback for rows whose owner truly
  cannot be reconstructed.

At the feature layer, `BudgetOwner` threads through the assignment read/write paths. `ResolutionContext` keys its
override maps on `(BudgetOwner, entry_id)`; `override_for` and `resolve` take the owner; `monthly_activity` tags each
journal/transaction row with its owner (`Character(character_id)` or `Corporation(corp_id)`) and resolves overrides
owner-aware. The wallet chip carries the row's owner through the picker and the cascade-to-twin write so an assignment
and its market/journal twin share one owner.

## Consequences

- Under the All scope, a character entry and a corporation entry that share an EVE id are distinct assignments and route
  to their own categories; category totals no longer mis-bucket cross-owner id collisions. This closes the cross-owner
  half of issue #38 (the market journal-twin suppression and transaction-id de-dup are a separate concern).
- The assignment uniqueness key widened; the upsert `ON CONFLICT` target tracks it. Existing assignments survive the
  migration with their owner reconstructed from the scope or the ledger; only un-reconstructable All-scope rows fall to
  the inert sentinel.
- Callers must supply an owner when assigning, clearing, or resolving an entry. Today the wallet surfaces only character
  rows, so the owner is `Character(character_id)`; the All-Wallets corp-row surfacing (Child A) sets the corporation
  owner on the rows it adds, and bulk assign (Child C) consumes the same identity.
