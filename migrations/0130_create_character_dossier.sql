CREATE TABLE IF NOT EXISTS character_dossier (
  character_id INTEGER NOT NULL PRIMARY KEY REFERENCES characters(id) ON DELETE CASCADE,
  purpose      TEXT,
  near_term    TEXT,
  created_at   TEXT    NOT NULL,
  updated_at   TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS dossier_orders (
  id           INTEGER NOT NULL PRIMARY KEY,
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  text         TEXT    NOT NULL,
  status       TEXT    NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'complete', 'cancelled')),
  objective_id INTEGER REFERENCES objectives(id) ON DELETE SET NULL,
  position     INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT    NOT NULL,
  updated_at   TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_dossier_orders_character ON dossier_orders(character_id);
CREATE INDEX IF NOT EXISTS idx_dossier_orders_objective ON dossier_orders(objective_id);
