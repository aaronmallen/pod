-- One-time historical repair of budget data. The writer-side co-assignment fix
-- and the first-run disposition code are already live; this migration only heals
-- rows that pre-date them and removes accumulated cruft, leaving referential
-- integrity clean. Every statement is idempotent and re-running it is a no-op
-- once the data is clean, so it is safe across a partially-migrated database.

-- ---------------------------------------------------------------------------
-- 1. Re-key mis-owned per-entry assignments.
--
-- 214 All-scope journal/market overrides were stamped owner_kind='character'
-- but their entry_id is a corporation-journal/transaction id (loaders stamp
-- corp rows as Corporation, and the override map is keyed (owner, entry_id), so
-- these rows match no live entry and are silently inert). Re-key them to the
-- holding corp owner so the user's intent is restored. We only touch rows whose
-- id exists in a corp ledger but NOT in the same character's ledger, so a
-- genuine character override that happens to share an EVE id is left alone.
UPDATE budget_entry_assignments
SET owner_kind = 'corporation',
    owner_id = (
      SELECT cwj.corporation_id FROM corporation_wallet_journal cwj
      WHERE cwj.id = budget_entry_assignments.entry_id
    ),
    updated_at = COALESCE(updated_at, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
WHERE scope_kind = 'all'
  AND entry_kind = 'journal'
  AND owner_kind = 'character'
  AND EXISTS (SELECT 1 FROM corporation_wallet_journal cwj WHERE cwj.id = budget_entry_assignments.entry_id)
  AND NOT EXISTS (
    SELECT 1 FROM character_wallet_journal cwj
    WHERE cwj.id = budget_entry_assignments.entry_id
      AND cwj.character_id = budget_entry_assignments.owner_id
  )
  -- Do not collide with an assignment already correctly keyed to that corp owner.
  AND NOT EXISTS (
    SELECT 1 FROM budget_entry_assignments dup
    WHERE dup.scope_kind = budget_entry_assignments.scope_kind
      AND dup.scope_id IS budget_entry_assignments.scope_id
      AND dup.entry_kind = budget_entry_assignments.entry_kind
      AND dup.entry_id = budget_entry_assignments.entry_id
      AND dup.owner_kind = 'corporation'
      AND dup.owner_id = (
        SELECT cwj.corporation_id FROM corporation_wallet_journal cwj
        WHERE cwj.id = budget_entry_assignments.entry_id
      )
  );

UPDATE budget_entry_assignments
SET owner_kind = 'corporation',
    owner_id = (
      SELECT cwt.corporation_id FROM corporation_wallet_transaction cwt
      WHERE cwt.transaction_id = budget_entry_assignments.entry_id
    ),
    updated_at = COALESCE(updated_at, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
WHERE scope_kind = 'all'
  AND entry_kind = 'market'
  AND owner_kind = 'character'
  AND EXISTS (SELECT 1 FROM corporation_wallet_transaction cwt WHERE cwt.transaction_id = budget_entry_assignments.entry_id)
  AND NOT EXISTS (
    SELECT 1 FROM character_wallet_transaction cwt
    WHERE cwt.transaction_id = budget_entry_assignments.entry_id
      AND cwt.character_id = budget_entry_assignments.owner_id
  )
  AND NOT EXISTS (
    SELECT 1 FROM budget_entry_assignments dup
    WHERE dup.scope_kind = budget_entry_assignments.scope_kind
      AND dup.scope_id IS budget_entry_assignments.scope_id
      AND dup.entry_kind = budget_entry_assignments.entry_kind
      AND dup.entry_id = budget_entry_assignments.entry_id
      AND dup.owner_kind = 'corporation'
      AND dup.owner_id = (
        SELECT cwt.corporation_id FROM corporation_wallet_transaction cwt
        WHERE cwt.transaction_id = budget_entry_assignments.entry_id
      )
  );

-- ---------------------------------------------------------------------------
-- 2. Collapse cross-owner duplicate assignments.
--
-- 333 ids carried an assignment under both owners (a character row + a corp row
-- for the same All-scope entry id). They aliased onto one entry before the
-- owner key existed; only one is real. Keep the newest by updated_at (the user's
-- last edit) and drop the rest. Ties break on the larger id (latest row).
-- This also subsumes any conflicting-category pairs (the newer category wins).
DELETE FROM budget_entry_assignments
WHERE id IN (
  SELECT a.id
  FROM budget_entry_assignments a
  JOIN budget_entry_assignments b
    ON a.scope_kind = b.scope_kind
    AND a.scope_id IS b.scope_id
    AND a.entry_kind = b.entry_kind
    AND a.entry_id = b.entry_id
    AND a.id <> b.id
  WHERE a.scope_kind = 'all'
    AND (
      b.updated_at > a.updated_at
      OR (b.updated_at = a.updated_at AND b.id > a.id)
    )
);

-- ---------------------------------------------------------------------------
-- 3. Drop unreachable scoped ref_type maps.
--
-- 68 ref_type maps were written under a character/corporation scope, but the
-- budget always resolves under the All scope, so a scoped map is never read.
-- Only All-scope ref_type maps are reachable.
DELETE FROM budget_ref_type_maps
WHERE scope_kind <> 'all' OR scope_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- 4. Drop abandoned char/corp-scoped category & group seeds.
--
-- Categories and groups are only ever created under the All scope today; any
-- character/corporation-scoped group is dead seed data. Deleting the group
-- cascades to its categories (FK ON DELETE CASCADE), which in turn cascades to
-- targets, assignments, per-entry overrides, ref_type maps and rules tied to
-- those categories — so referential integrity stays clean. We delete in the
-- same direction (groups → cascade) and also clear the matching seed markers.
DELETE FROM budget_category_groups
WHERE scope_kind <> 'all' OR scope_id IS NOT NULL;

DELETE FROM budget_scope_seeded
WHERE scope_kind <> 'all' OR scope_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- 5. Remove orphan per-entry overrides whose entry no longer exists.
--
-- 13 overrides point at an entry_id absent from every live ledger for their
-- owner kind (the wallet row was pruned). They can never resolve, so drop them.
DELETE FROM budget_entry_assignments
WHERE entry_kind = 'journal'
  AND owner_kind = 'character'
  AND NOT EXISTS (
    SELECT 1 FROM character_wallet_journal cwj
    WHERE cwj.id = budget_entry_assignments.entry_id
      AND cwj.character_id = budget_entry_assignments.owner_id
  );

DELETE FROM budget_entry_assignments
WHERE entry_kind = 'journal'
  AND owner_kind = 'corporation'
  AND NOT EXISTS (
    SELECT 1 FROM corporation_wallet_journal cwj
    WHERE cwj.id = budget_entry_assignments.entry_id
      AND cwj.corporation_id = budget_entry_assignments.owner_id
  );

DELETE FROM budget_entry_assignments
WHERE entry_kind = 'market'
  AND owner_kind = 'character'
  AND NOT EXISTS (
    SELECT 1 FROM character_wallet_transaction cwt
    WHERE cwt.transaction_id = budget_entry_assignments.entry_id
      AND cwt.character_id = budget_entry_assignments.owner_id
  );

DELETE FROM budget_entry_assignments
WHERE entry_kind = 'market'
  AND owner_kind = 'corporation'
  AND NOT EXISTS (
    SELECT 1 FROM corporation_wallet_transaction cwt
    WHERE cwt.transaction_id = budget_entry_assignments.entry_id
      AND cwt.corporation_id = budget_entry_assignments.owner_id
  );

-- ---------------------------------------------------------------------------
-- 6. Remove legacy owner_id = 0 rows.
--
-- 5 rows kept the 0099 sentinel owner_id = 0: their owner could not be derived
-- at backfill, so they match no live entry. Anything genuinely re-keyable was
-- already healed in step 1; the rest are dead and removed here.
DELETE FROM budget_entry_assignments
WHERE owner_id = 0;

-- ---------------------------------------------------------------------------
-- 7. Remove zero-value monthly targets.
--
-- 11 targets carry amount = 0 (a cleared goal that was never deleted). A zero
-- target is indistinguishable from no target to the UI, so prune them.
DELETE FROM budget_targets
WHERE amount = 0;
