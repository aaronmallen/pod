-- One-time backfill of the per-owner assignment copies the broken mark-time
-- cascade failed to write. A corp-on-behalf trade lands in the fast character
-- wallet and the slow corp journal at very different times; the in-memory cascade
-- only filled copies already loaded, so any sibling leg that synced later was
-- left unmarked. The forward-only post-sync reconciler heals such events the next
-- time they are touched, but pre-existing split marks need this one-time repair.
--
-- This is the migration mirror of the runtime reconciler
-- (budget::RECONCILE_SPLIT_OWNER_ASSIGNMENTS_SQL): same linkage, fill-only, no
-- collapse and no re-key (0099/0102 already healed the historical aliasing). The
-- `legs` CTE is every concrete wallet leg of a market event keyed by its trade
-- `transaction_id` — a market row by its own `transaction_id`; a journal twin
-- (`market_transaction`) and the broker-fee / transaction-tax legs by `context_id`,
-- which EVE sets to the trade's `transaction_id`. Each missing sibling copy is
-- materialized under the category of the newest source mark in the group.
--
-- Idempotent and re-runnable: the `NOT EXISTS` guard plus the owner-aware unique
-- index from migration 0099 (ON CONFLICT ... DO NOTHING) make a second pass a
-- no-op. Override-respecting: an owner that already holds its own assignment is
-- never overwritten, and an owner that deliberately unassigned can only be
-- re-filled by a strictly newer source mark, never resurrected by a stale one.
INSERT INTO budget_entry_assignments
  (scope_kind, scope_id, owner_kind, owner_id, entry_kind, entry_id, category_id, created_at, updated_at)
WITH legs AS (
  SELECT transaction_id AS transaction_id, 'character' AS owner_kind, character_id AS owner_id,
         'market' AS entry_kind, transaction_id AS entry_id
    FROM character_wallet_transaction
  UNION ALL
  SELECT transaction_id, 'corporation', corporation_id, 'market', transaction_id
    FROM corporation_wallet_transaction
  UNION ALL
  SELECT context_id, 'character', character_id, 'journal', id
    FROM character_wallet_journal
   WHERE context_id IS NOT NULL
     AND ref_type IN ('market_transaction', 'brokers_fee', 'transaction_tax')
  UNION ALL
  SELECT context_id, 'corporation', corporation_id, 'journal', id
    FROM corporation_wallet_journal
   WHERE context_id IS NOT NULL
     AND ref_type IN ('market_transaction', 'brokers_fee', 'transaction_tax')
),
sources AS (
  SELECT l.transaction_id AS transaction_id, a.category_id AS category_id, a.updated_at AS updated_at, a.id AS id
    FROM budget_entry_assignments a
    JOIN legs l
      ON l.owner_kind = a.owner_kind AND l.owner_id = a.owner_id
     AND l.entry_kind = a.entry_kind AND l.entry_id = a.entry_id
   WHERE a.scope_kind = 'all' AND a.scope_id IS NULL
),
winners AS (
  SELECT s.transaction_id AS transaction_id, s.category_id AS category_id
    FROM sources s
    JOIN (
      SELECT transaction_id, MAX(updated_at) AS updated_at FROM sources GROUP BY transaction_id
    ) latest ON latest.transaction_id = s.transaction_id AND latest.updated_at = s.updated_at
    JOIN (
      SELECT transaction_id, updated_at, MAX(id) AS id FROM sources GROUP BY transaction_id, updated_at
    ) tiebreak ON tiebreak.transaction_id = s.transaction_id AND tiebreak.updated_at = s.updated_at
              AND tiebreak.id = s.id
)
SELECT 'all', NULL, l.owner_kind, l.owner_id, l.entry_kind, l.entry_id, w.category_id,
       strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
  FROM legs l
  JOIN winners w ON w.transaction_id = l.transaction_id
 WHERE NOT EXISTS (
   SELECT 1 FROM budget_entry_assignments a
    WHERE a.scope_kind = 'all' AND a.scope_id IS NULL
      AND a.owner_kind = l.owner_kind AND a.owner_id = l.owner_id
      AND a.entry_kind = l.entry_kind AND a.entry_id = l.entry_id
 )
 GROUP BY l.owner_kind, l.owner_id, l.entry_kind, l.entry_id, w.category_id
ON CONFLICT(scope_kind, COALESCE(scope_id, -1), owner_kind, owner_id, entry_kind, entry_id) DO NOTHING;
