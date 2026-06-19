CREATE TABLE IF NOT EXISTS budget_scope_seeded (
  scope_kind TEXT    NOT NULL,
  scope_id   INTEGER,
  seeded_at  TEXT    NOT NULL
);

-- COALESCE(scope_id, -1) so an All-scope (NULL scope_id) marks a single row;
-- a plain UNIQUE treats NULLs as distinct and would let the All scope re-seed.
CREATE UNIQUE INDEX IF NOT EXISTS idx_budget_scope_seeded_unique
  ON budget_scope_seeded(scope_kind, COALESCE(scope_id, -1));
