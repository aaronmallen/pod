CREATE TABLE IF NOT EXISTS budget_category_groups (
  id         INTEGER PRIMARY KEY NOT NULL,
  scope_kind TEXT    NOT NULL,
  scope_id   INTEGER,
  name       TEXT    NOT NULL,
  position   INTEGER NOT NULL DEFAULT 0,
  created_at TEXT    NOT NULL,
  updated_at TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS budget_categories (
  id         INTEGER PRIMARY KEY NOT NULL,
  group_id   INTEGER NOT NULL REFERENCES budget_category_groups(id) ON DELETE CASCADE,
  name       TEXT    NOT NULL,
  note       TEXT,
  tone       TEXT,
  position   INTEGER NOT NULL DEFAULT 0,
  created_at TEXT    NOT NULL,
  updated_at TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS budget_targets (
  category_id INTEGER PRIMARY KEY NOT NULL REFERENCES budget_categories(id) ON DELETE CASCADE,
  kind        TEXT    NOT NULL,
  amount      REAL    NOT NULL,
  by_date     TEXT
);

CREATE TABLE IF NOT EXISTS budget_assignments (
  id          INTEGER PRIMARY KEY NOT NULL,
  category_id INTEGER NOT NULL REFERENCES budget_categories(id) ON DELETE CASCADE,
  month       TEXT    NOT NULL,
  assigned    REAL    NOT NULL,
  UNIQUE(category_id, month)
);

CREATE TABLE IF NOT EXISTS budget_ref_type_maps (
  id          INTEGER PRIMARY KEY NOT NULL,
  scope_kind  TEXT    NOT NULL,
  scope_id    INTEGER,
  ref_type    TEXT    NOT NULL,
  category_id INTEGER NOT NULL REFERENCES budget_categories(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_budget_category_groups_scope ON budget_category_groups(scope_kind, scope_id);
CREATE INDEX IF NOT EXISTS idx_budget_categories_group_id ON budget_categories(group_id);
CREATE INDEX IF NOT EXISTS idx_budget_assignments_category_month ON budget_assignments(category_id, month);

-- COALESCE(scope_id, -1) so an All-scope (NULL scope_id) ref_type maps to a single row;
-- SQLite treats NULLs as distinct in a plain UNIQUE, which would defeat upsert for the All scope.
CREATE UNIQUE INDEX IF NOT EXISTS idx_budget_ref_type_maps_unique
  ON budget_ref_type_maps(scope_kind, COALESCE(scope_id, -1), ref_type);
