-- Budget automation rules: durable, ordered, scope-keyed search filters that auto-file matching
-- ledger outflows into a budget envelope. Priority is row order (position). Rules live under the
-- All scope today (scope_kind/scope_id mirror every other budget table so the loader stays
-- scope-agnostic for the engine in child B). This replaces the design's localStorage JSON.

CREATE TABLE IF NOT EXISTS budget_rules (
  id          INTEGER PRIMARY KEY NOT NULL,
  scope_kind  TEXT    NOT NULL,
  scope_id    INTEGER,
  category_id INTEGER NOT NULL REFERENCES budget_categories(id) ON DELETE CASCADE,
  name        TEXT    NOT NULL,
  enabled     INTEGER NOT NULL DEFAULT 1,
  match_mode  TEXT    NOT NULL DEFAULT 'all' CHECK (match_mode IN ('all', 'any')),
  position    INTEGER NOT NULL DEFAULT 0,
  created_at  TEXT    NOT NULL,
  updated_at  TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS budget_rule_conditions (
  id       INTEGER PRIMARY KEY NOT NULL,
  rule_id  INTEGER NOT NULL REFERENCES budget_rules(id) ON DELETE CASCADE,
  field    TEXT    NOT NULL,
  op       TEXT    NOT NULL,
  value    TEXT    NOT NULL,
  value2   TEXT,
  position INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_budget_rules_scope ON budget_rules(scope_kind, scope_id, position);
CREATE INDEX IF NOT EXISTS idx_budget_rule_conditions_rule ON budget_rule_conditions(rule_id, position);
