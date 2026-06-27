---
id: "0040"
title: Per-Wallet Journal Identity
status: active
tags: [data-model, finance, sync, persistence, budget]
created: 2026-06-26
---

# ADR-0040: Per-Wallet Journal Identity

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

EVE wallet journals and transactions are fetched per wallet (per character, and per corporation wallet division), but
the four storage tables key each row on the bare ESI identifier alone: `character_wallet_journal` and
`corporation_wallet_journal` use `PRIMARY KEY (id)`, and `character_wallet_transaction` and
`corporation_wallet_transaction` use `PRIMARY KEY (transaction_id)`. An internal transfer (ISK moved between two
corporation wallet divisions, or between a character and a corporation) produces two journal legs that share the same
ESI `ref_id`. Because the corporation tables key globally rather than per division, the second leg collides with the
first under `ON CONFLICT(id) DO NOTHING` and is silently dropped, so transfer legs go missing and balances and budget
totals drift. We re-key all four tables onto a per-wallet composite identity, so both legs of a transfer survive. A
one-time re-key migration (`0110`) rebuilds the tables onto the new keys without losing existing rows, a one-time forced
re-fetch re-pulls the legs ESI still serves, internal-transfer budget semantics are confirmed to be already correct
once both legs persist, and a balance-continuity check reports remaining gaps the re-fetch cannot fill.

## Context

ESI exposes the EVE journal `id` and transaction `transaction_id` as the natural keys of a wallet, but those ids are
unique only *within a single wallet*, not across an owner's wallets and not across owners. ADR-0028 already established
this for the budget layer: a character copy and a corporation copy of the same event share an EVE id, so budget
assignments must key on `(owner, entry_id)` rather than `entry_id` alone. The same fact has been left unhandled one
level down, in the raw ledger tables.

The four tables and their current keys:

- `character_wallet_journal`: `PRIMARY KEY (id)`, written by `finance::append_wallet_journal` with
  `ON CONFLICT(id) DO NOTHING`.
- `character_wallet_transaction`: `PRIMARY KEY (transaction_id)`, written by `finance::append_wallet_transaction` with
  `ON CONFLICT(transaction_id) DO NOTHING`.
- `corporation_wallet_journal`: `PRIMARY KEY (id)`, carries a `division` column, written by
  `finance::append_corporation_wallet_journal` with `ON CONFLICT(id) DO NOTHING`.
- `corporation_wallet_transaction`: `PRIMARY KEY (transaction_id)`, carries a `division` column, written by
  `finance::append_corporation_wallet_transaction` with `ON CONFLICT(transaction_id) DO NOTHING`.

The corporation case is where the loss happens today. `sync::jobs::corporation_wallet::sync_division` loops divisions
`1..=7` and appends each division's journal and transactions into a single table keyed on the bare id. When the same
ISK movement appears in two divisions of one corporation (an internal transfer, e.g. `corporation_account_withdrawal`),
both rows carry the same EVE `id` but a different `division`. The first division's row inserts; the second collides on
the `id` primary key and is dropped. One half of the transfer never reaches the database, and every downstream
consumer (the journal view, the daily liquid backfill `corporation_backfill_liquid_from_journal`, and the budget
engine) sees a one-sided event.

The character case is currently masked but structurally fragile. Today a single character has one wallet, so
`(character_id, id)` and `(id)` coincide and no row is lost. But the budget layer and the cross-owner reconciler
(ADR-0035) already treat the *character versus corporation* axis as the thing that disambiguates a shared id, and the
character table's bare-id key encodes the assumption that an id is globally unique for a character. Aligning the
character tables to the same per-wallet composite removes the latent assumption and keeps all four tables on one rule.

The budget layer is already correct *given complete data*. `budget_engine` keys every journal row by `(owner, id)`,
internal-transfer detection (`internal_transfer_ids`) groups legs by `(ref_type, id)` and requires exactly two legs of
opposite sign in distinct owners that net to within 0.5 ISK, and a confirmed transfer marks both legs
`BudgetFlow::InternalTransfer` so neither contributes to activity or ready-to-assign. The bug is not in budget logic;
it is that the second leg never reaches the table, so the pair can never be detected and the surviving leg is counted as
real income or spend. Fixing identity at the storage layer is therefore sufficient for budgets once both legs persist.

## Decision

### 1. Per-wallet composite identity

Re-key all four ledger tables so the row identity is the owning wallet plus the ESI id, never the ESI id alone.

| Table                            | New primary key                              | Wallet columns               |
| -------------------------------- | -------------------------------------------- | ---------------------------- |
| `character_wallet_journal`       | `(character_id, id)`                         | `character_id`               |
| `character_wallet_transaction`   | `(character_id, transaction_id)`             | `character_id`               |
| `corporation_wallet_journal`     | `(corporation_id, division, id)`             | `corporation_id`, `division` |
| `corporation_wallet_transaction` | `(corporation_id, division, transaction_id)` | `corporation_id`, `division` |

The corporation key includes `division` because the wallet of a corporation is the *division*, not the corporation as a
whole: the same id can legitimately appear once per division for an internal transfer, and each must be a distinct row.
The character key is `(character_id, id)` because a character's wallet is the character.

The four upserts change their conflict targets accordingly:

- `append_wallet_journal`: `ON CONFLICT(character_id, id) DO NOTHING`.
- `append_wallet_transaction`: `ON CONFLICT(character_id, transaction_id) DO NOTHING`.
- `append_corporation_wallet_journal`: `ON CONFLICT(corporation_id, division, id) DO NOTHING`.
- `append_corporation_wallet_transaction`: `ON CONFLICT(corporation_id, division, transaction_id) DO NOTHING`.

`DO NOTHING` is retained: re-fetching a wallet that already holds a row for `(wallet, id)` must remain a cheap no-op, so
re-sync stays idempotent.

The ESI `id` / `transaction_id` columns stay on every row unchanged. The budget layer references a journal row by
`(owner_kind, owner_id, entry_id)` where `entry_id` is exactly this id, so preserving the id value keeps every existing
budget assignment, ref-type map, and the ADR-0035 reconciler working with no change to budget code or data.

### 2. Re-key migration (`0110`)

SQLite cannot redefine a primary key in place, so the migration uses the standard table-rebuild for each of the four
tables, inside the single migration transaction:

1. `CREATE TABLE <name>_new (...)` with the new composite `PRIMARY KEY` and the existing column set, foreign keys, and
   indexes carried forward.
2. `INSERT OR IGNORE INTO <name>_new SELECT ... FROM <name>` to copy every existing row. `OR IGNORE` makes the copy
   tolerant of any historical duplicate that the old bare-id key would have rejected anyway, so the rebuild can never
   fail on legacy data.
3. `DROP TABLE <name>` then `ALTER TABLE <name>_new RENAME TO <name>`.
4. Recreate the secondary indexes that migrations 0034/0035/0040/0041/0088 defined on the table (`character_id`,
   `(character_id, id)`, `journal_ref_id`, `type_id`, `(corporation_id, division)`, the `char_tx` hot-path index, etc.).

The migration only restores *identity*; it cannot recover a corporation transfer leg that was dropped before it ever
reached the database. Those legs are recovered by the forced re-fetch in section 4, bounded by what ESI still serves.

Migration-safety rules this migration must honor:

- It is purely additive to the migration set (new file `0110`), so it does not touch the bytes of any already-released
  migration and cannot trip the sqlx embedded-checksum guard that the 0.6.8 CRLF heal addresses. Existing migrations
  `0001..0109` keep their checksums; only a new row is appended to `_sqlx_migrations`.
- Every statement is idempotent in the sense that the migration runs exactly once per database (sqlx version-gates it),
  and the rebuild leaves a schema indistinguishable from a fresh install, so a fresh install and an upgraded install
  converge on the identical schema.
- It runs inside sqlx's per-migration transaction, so a crash mid-rebuild rolls back to the pre-migration tables rather
  than leaving a half-renamed table.

### 3. Internal-transfer budget semantics

No new budget code is required; the existing semantics become correct once both legs persist. This ADR records the
contract so the migration and re-fetch tasks do not accidentally regress it:

- An internal transfer is detected, not declared. `internal_transfer_ids` pairs two legs that share `(ref_type, id)`,
  have opposite signs, belong to distinct owners, and net to within `TRANSFER_NET_EPSILON` (0.5 ISK). Detection
  therefore *requires both legs in the table*, which is exactly what this ADR restores for corporation divisions.
- Both legs of a confirmed transfer are classified `BudgetFlow::InternalTransfer` and excluded from monthly activity and
  from ready-to-assign, so a transfer contributes zero net to the budget. Persisting the second leg does not
  double-count; it is what lets the pair cancel. Before this fix, the surviving single leg was misclassified as real
  income or spend.
- The detection key is per-id within the ambiguous ref-type set (`contract_price`,
  `corporation_account_withdrawal`, `player_donation`, `player_trading`). Because corporation division legs share the
  same `id`, two divisions of one corporation that move ISK between themselves will now present both legs under the same
  id and net correctly. The downstream "internal-transfer budget semantics" task should add a test that two corp
  division legs of one transfer are both persisted and jointly classified as a transfer (the regression this whole
  iteration exists to prevent).

### 4. One-time forced re-fetch of wallet journals on upgrade

The re-key migration restores the ability to store both legs, but legs already dropped are simply absent. A separate
one-time forced re-fetch re-pulls them from ESI:

- Mechanism: reset the wallet jobs' freshness so they re-run immediately and re-pull, rather than waiting out their
  normal interval. The sync ledger (ADR-0014) stores per-`(subject, kind)` freshness in `sync_ledger`; deleting the
  `CharacterWallet` and `CorporationWallet` rows (or setting `next_eligible_at` to the past) makes those jobs present as
  never-attempted, so they re-fetch on the next pass and re-append through the now-per-wallet upserts. ESI fetches are
  fully paginated (`get_json_paginated` follows `X-Pages`), so the re-fetch pulls every page ESI currently returns.
- Bounding: this only recovers legs still inside ESI's journal retention window (historically about 30 days, around
  2500 entries per wallet). Legs older than that window are unrecoverable from ESI and remain gaps; section 5 detects
  and reports them rather than pretending they were healed.
- One-time guard: the reset must fire exactly once on upgrade, not on every launch. Carry it in the same migration
  (`0110`) as a ledger-row delete for the two wallet kinds, following the one-time-repair precedent of migrations 0102
  and 0104. A migration runs once per database, so the re-fetch is triggered once; subsequent syncs follow the normal
  freshness cadence. Because the upserts are `DO NOTHING`, the re-fetch is safe even if a row already exists.

### 5. Balance-continuity gap detection

Each journal row carries `balance`, the running wallet balance immediately after the entry. For a complete, correctly
ordered ledger, consecutive entries within one wallet satisfy `balance(n) - balance(n-1) == amount(n)` (within float
tolerance). A break in that identity means an entry is missing: a dropped transfer leg, or a page ESI no longer
serves.

- Scope and ordering: evaluate per wallet (`character_id`, or `corporation_id` + `division`), ordered by `id` ascending.
  EVE journal ids are monotonic per wallet, so id order is balance order; this is the same ordering the existing
  `wallet_journal` and `corporation_wallet_journal` queries already rely on.
- Check: walk adjacent rows whose `balance` and `amount` are both non-null and flag any pair where
  `abs((balance(n) - balance(n-1)) - amount(n))` exceeds a small ISK epsilon. Rows with null `balance`/`amount` (some
  ref types omit them) are skipped, not treated as gaps.
- Surfacing: emit a notification via `notifications::emit` with a stable `dedup_key` per wallet-and-gap so the warning
  fires once and does not re-nag after the 90-day notification retention tombstones it (ADR-0037). The notification
  tells the pilot a wallet has a balance discontinuity that pre-dates ESI's retention window and cannot be auto-healed,
  distinguishing a real data gap from the transient one the re-fetch just closed.
- Timing: run the check after the forced re-fetch settles, so gaps the re-fetch can close are not reported.

## Affected Areas

- `migrations/0110_*.sql`: new migration; rebuild all four ledger tables onto the per-wallet composite keys, carry
  forward indexes, and delete the `CharacterWallet` / `CorporationWallet` sync-ledger rows to force the one-time
  re-fetch.
- `src/store/repo/finance.rs`: the four append upserts change their `ON CONFLICT` targets to the composite keys;
  `corporation_backfill_liquid_from_journal` and `backfill_liquid_from_journal` keep working unchanged once both legs
  are present.
- `src/sync/jobs/character_wallet.rs`, `src/sync/jobs/corporation_wallet.rs`: no key changes needed (they already pass
  `character_id` / `(corporation_id, division)`); add the balance-continuity check and its notification, run after
  re-fetch.
- `src/store/model/{character,corporation}_wallet_{journal,transaction}.rs`: unchanged (the id columns are retained).
- `src/features/wallet/budget_engine.rs`: unchanged; this ADR confirms its transfer semantics are correct once data is
  complete and asks the downstream task to add the two-corp-division regression test.

## Consequences

### Positive

- Both legs of an internal transfer survive, so corporation division-to-division and player-to-corp transfers stop
  vanishing, and balances, the daily liquid backfill, and budget totals reconcile.
- Budget internal-transfer detection finally has both legs to pair, so transfers net to zero instead of being
  miscounted as income or spend, with no change to budget code.
- All four ledger tables converge on one identity rule that matches the owner-aware model ADR-0028 already established
  one level up, removing a latent global-uniqueness assumption from the character tables.
- The migration is additive, so it sidesteps the sqlx embedded-checksum hazard; the forced re-fetch and the gap check
  together heal what is recoverable and honestly report what is not.

### Negative

- The re-key migration rebuilds four tables, which on a large wallet history is a one-time copy cost at upgrade
  (bounded; these tables are small relative to assets).
- The forced re-fetch only recovers legs inside ESI's retention window; legs older than roughly 30 days stay missing and
  are reported as gaps rather than healed.
- Balance-continuity warnings may surface pre-existing historical gaps that predate the fix and can never be closed; the
  notification must frame these as informational, not as a recurring error.

## Open Questions

- Balance epsilon for the continuity check (fixed ISK tolerance vs relative)?
- Whether the gap notification should be per wallet or aggregated to one per owner to avoid noise on accounts with many
  corp divisions.
- Whether to also expose detected gaps in the wallet UI (a row marker) or keep them notification-only for v1.

## Future Work

- A first-class logical-event identity that unifies the two legs of a transfer into one envelope (rather than detecting
  the pair heuristically by `(ref_type, id)` and netting) is deferred; it would build on the same per-wallet rows this
  ADR establishes. This dovetails with the canonical logical-event identity already noted as future work in ADR-0035.

## References

- ADR-0028 (owner-aware budget assignment identity): established `(owner, entry_id)` keying for the budget layer; this
  ADR pushes the same per-wallet rule down into the raw ledger tables.
- ADR-0035 (post-sync cross-owner budget mark reconciliation): relies on per-owner journal rows and the retained EVE
  ids that this ADR preserves.
- ADR-0014 (persisted sync ledger) and ADR-0036 (freshness-first sync): the `sync_ledger` freshness reset used for the
  one-time forced re-fetch.
- ADR-0037 (notification history): the `notifications::emit` dedup/retention model used for gap-detection warnings.
- Migrations 0102 and 0104: one-time idempotent data-repair precedent for the upgrade-time reset.
- Existing storage: `migrations/0034`, `0035`, `0040`, `0041`, `0088`; upserts in `src/store/repo/finance.rs`;
  transfer detection in `src/features/wallet/budget_engine.rs`.
