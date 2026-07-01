-- Collapse the budget on-disk model onto a journal-only, scope-free shape (spec trrvzrsx).
-- Assignment identity becomes (owner_kind, owner_id, journal id) -> category: there is no
-- scope (every budget row was already the 'all' scope) and no journal/market entry_kind
-- duality (a market override is folded onto its journal twin). The data repair is one-time
-- (0102 precedent) and sqlx runs this migration once per database.

-- ---------------------------------------------------------------------------
-- 0. Drop the scope/entry_kind identity indexes up front.
--
-- Folding a market override onto its journal twin (step 1) re-points entry_id and can alias
-- onto an existing journal override, so the old (scope, owner, entry_kind, entry_id) unique
-- index must go before the data moves; the journal-only unique index is rebuilt in step 4.
DROP INDEX IF EXISTS idx_budget_entry_assignments_unique;
DROP INDEX IF EXISTS idx_budget_entry_assignments_scope;

-- ---------------------------------------------------------------------------
-- 1. Fold market overrides onto their journal twin.
--
-- A market override keys on a transaction_id (entry_kind='market', entry_id=transaction_id).
-- Its journal twin is the ledger row with ref_type='market_transaction' whose context_id is
-- that transaction_id (per-owner ledger identity, ADR-0040). Re-point entry_id at the twin's
-- journal id and restamp the row as a journal override. Only rows whose twin exists are folded.
UPDATE budget_entry_assignments
SET entry_id = (
      SELECT cwj.id
      FROM character_wallet_journal cwj
      WHERE cwj.character_id = budget_entry_assignments.owner_id
        AND cwj.ref_type = 'market_transaction'
        AND cwj.context_id = budget_entry_assignments.entry_id
    ),
    entry_kind = 'journal',
    updated_at = COALESCE(updated_at, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
WHERE entry_kind = 'market'
  AND owner_kind = 'character'
  AND EXISTS (
    SELECT 1
    FROM character_wallet_journal cwj
    WHERE cwj.character_id = budget_entry_assignments.owner_id
      AND cwj.ref_type = 'market_transaction'
      AND cwj.context_id = budget_entry_assignments.entry_id
  );

UPDATE budget_entry_assignments
SET entry_id = (
      SELECT cwj.id
      FROM corporation_wallet_journal cwj
      WHERE cwj.corporation_id = budget_entry_assignments.owner_id
        AND cwj.ref_type = 'market_transaction'
        AND cwj.context_id = budget_entry_assignments.entry_id
    ),
    entry_kind = 'journal',
    updated_at = COALESCE(updated_at, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
WHERE entry_kind = 'market'
  AND owner_kind = 'corporation'
  AND EXISTS (
    SELECT 1
    FROM corporation_wallet_journal cwj
    WHERE cwj.corporation_id = budget_entry_assignments.owner_id
      AND cwj.ref_type = 'market_transaction'
      AND cwj.context_id = budget_entry_assignments.entry_id
  );

-- ---------------------------------------------------------------------------
-- 2. Drop unresolvable overrides.
--
-- Any remaining market override could not be folded (its journal twin was pruned from the
-- ledger), so it can never resolve under the journal-only model. Drop it. The row count
-- removed here is the migration's dropped-override tally (0102 precedent).
DELETE FROM budget_entry_assignments
WHERE entry_kind = 'market';

-- ---------------------------------------------------------------------------
-- 3. Collapse duplicate overrides onto one row per (owner, journal id).
--
-- Dropping scope and folding market twins can leave two overrides for the same owner and
-- journal id (a scoped legacy copy, or a market fold landing on a journal twin that already
-- carried an override). Keep the newest by updated_at, breaking ties on the larger id (the
-- user's last edit), and drop the rest. Multi-owner cascade copies keep distinct owner_id
-- values, so they survive as separate rows.
DELETE FROM budget_entry_assignments
WHERE id IN (
  SELECT a.id
  FROM budget_entry_assignments a
  JOIN budget_entry_assignments b
    ON a.owner_kind = b.owner_kind
    AND a.owner_id = b.owner_id
    AND a.entry_id = b.entry_id
    AND a.id <> b.id
  WHERE b.updated_at > a.updated_at
    OR (b.updated_at = a.updated_at AND b.id > a.id)
);

-- ---------------------------------------------------------------------------
-- 4. Drop the scope/entry_kind columns from budget_entry_assignments.
--
-- The scope/entry_kind indexes were already removed in step 0; the surviving identity is a
-- single unique index on (owner_kind, owner_id, entry_id).
ALTER TABLE budget_entry_assignments DROP COLUMN scope_kind;
ALTER TABLE budget_entry_assignments DROP COLUMN scope_id;
ALTER TABLE budget_entry_assignments DROP COLUMN entry_kind;

CREATE UNIQUE INDEX IF NOT EXISTS idx_budget_entry_assignments_unique
  ON budget_entry_assignments(owner_kind, owner_id, entry_id);

-- ---------------------------------------------------------------------------
-- 5. Drop the scope columns from budget_category_groups.
--
-- Groups only ever existed under the 'all' scope; the scope-keyed index goes with them.
DROP INDEX IF EXISTS idx_budget_category_groups_scope;

ALTER TABLE budget_category_groups DROP COLUMN scope_kind;
ALTER TABLE budget_category_groups DROP COLUMN scope_id;

-- ---------------------------------------------------------------------------
-- 6. Drop the scope columns from budget_rules.
--
-- Rules only ever ran under the 'all' scope; replace the scope-keyed ordering index with one
-- on position alone.
DROP INDEX IF EXISTS idx_budget_rules_scope;

ALTER TABLE budget_rules DROP COLUMN scope_kind;
ALTER TABLE budget_rules DROP COLUMN scope_id;

CREATE INDEX IF NOT EXISTS idx_budget_rules_position ON budget_rules(position);

-- ---------------------------------------------------------------------------
-- 7. Retire the scope-only budget tables.
--
-- budget_ref_type_maps was a scoped ref_type -> category map that the journal-only engine no
-- longer consults, and budget_scope_seeded tracked a per-scope first-run marker that has no
-- meaning without scopes. Both are dropped.
DROP INDEX IF EXISTS idx_budget_ref_type_maps_unique;
DROP TABLE IF EXISTS budget_ref_type_maps;

DROP INDEX IF EXISTS idx_budget_scope_seeded_unique;
DROP TABLE IF EXISTS budget_scope_seeded;
