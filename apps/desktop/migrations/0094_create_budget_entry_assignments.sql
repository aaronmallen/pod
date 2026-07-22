CREATE TABLE IF NOT EXISTS budget_entry_assignments (
  id          INTEGER PRIMARY KEY NOT NULL,
  scope_kind  TEXT    NOT NULL,
  scope_id    INTEGER,
  entry_kind  TEXT    NOT NULL,
  entry_id    INTEGER NOT NULL,
  category_id INTEGER NOT NULL REFERENCES budget_categories(id) ON DELETE CASCADE,
  created_at  TEXT    NOT NULL,
  updated_at  TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_budget_entry_assignments_scope
  ON budget_entry_assignments(scope_kind, scope_id);

-- COALESCE(scope_id, -1) so an All-scope (NULL scope_id) entry maps to a single row;
-- SQLite treats NULLs as distinct in a plain UNIQUE, which would defeat upsert for the All scope.
CREATE UNIQUE INDEX IF NOT EXISTS idx_budget_entry_assignments_unique
  ON budget_entry_assignments(scope_kind, COALESCE(scope_id, -1), entry_kind, entry_id);
