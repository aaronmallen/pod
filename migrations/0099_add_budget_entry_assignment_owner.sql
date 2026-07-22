-- Owner-aware budget assignment identity. EVE journal ref ids and transaction
-- ids are unique only per owner, so under the All scope (scope_id NULL) two
-- owners sharing an id aliased onto one assignment row. Adds owner_kind/owner_id
-- and folds them into the uniqueness key so a character entry and a corporation
-- entry sharing an EVE id are distinct assignments.

-- Defaults make the ADD COLUMN safe on existing rows; the backfill below replaces
-- them. The 'character'/0 default is the documented sentinel for any All-scope row
-- whose owner cannot be derived (see ADR-0028): the row is retained but matches no
-- live entry rather than being lost.
ALTER TABLE budget_entry_assignments ADD COLUMN owner_kind TEXT NOT NULL DEFAULT 'character';
ALTER TABLE budget_entry_assignments ADD COLUMN owner_id INTEGER NOT NULL DEFAULT 0;

-- Scoped rows were created under a known owner: the scope itself.
UPDATE budget_entry_assignments
SET owner_kind = scope_kind, owner_id = scope_id
WHERE scope_kind IN ('character', 'corporation') AND scope_id IS NOT NULL;

-- All-scope journal rows: derive the owner from whichever ledger holds the entry,
-- preferring the character wallet on the corp-mirror id collision.
UPDATE budget_entry_assignments
SET owner_kind = 'character',
    owner_id = (SELECT character_id FROM character_wallet_journal WHERE id = budget_entry_assignments.entry_id)
WHERE scope_kind = 'all' AND entry_kind = 'journal'
  AND EXISTS (SELECT 1 FROM character_wallet_journal WHERE id = budget_entry_assignments.entry_id);

UPDATE budget_entry_assignments
SET owner_kind = 'corporation',
    owner_id = (SELECT corporation_id FROM corporation_wallet_journal WHERE id = budget_entry_assignments.entry_id)
WHERE scope_kind = 'all' AND entry_kind = 'journal' AND owner_id = 0
  AND EXISTS (SELECT 1 FROM corporation_wallet_journal WHERE id = budget_entry_assignments.entry_id);

-- All-scope market rows: same derivation against the transaction ledgers.
UPDATE budget_entry_assignments
SET owner_kind = 'character',
    owner_id = (SELECT character_id FROM character_wallet_transaction WHERE transaction_id = budget_entry_assignments.entry_id)
WHERE scope_kind = 'all' AND entry_kind = 'market'
  AND EXISTS (SELECT 1 FROM character_wallet_transaction WHERE transaction_id = budget_entry_assignments.entry_id);

UPDATE budget_entry_assignments
SET owner_kind = 'corporation',
    owner_id = (SELECT corporation_id FROM corporation_wallet_transaction WHERE transaction_id = budget_entry_assignments.entry_id)
WHERE scope_kind = 'all' AND entry_kind = 'market' AND owner_id = 0
  AND EXISTS (SELECT 1 FROM corporation_wallet_transaction WHERE transaction_id = budget_entry_assignments.entry_id);

-- Replace the uniqueness key with one that includes the owner. COALESCE(scope_id, -1)
-- keeps the All scope a single row per (owner, entry); SQLite treats NULLs as
-- distinct in a plain UNIQUE, which would defeat the upsert.
DROP INDEX IF EXISTS idx_budget_entry_assignments_unique;

CREATE UNIQUE INDEX IF NOT EXISTS idx_budget_entry_assignments_unique
  ON budget_entry_assignments(scope_kind, COALESCE(scope_id, -1), owner_kind, owner_id, entry_kind, entry_id);
