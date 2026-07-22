CREATE TABLE IF NOT EXISTS sync_lists (
  id         INTEGER PRIMARY KEY,
  name       TEXT    NOT NULL DEFAULT 'Untitled list',
  created_at TEXT    NOT NULL,
  updated_at TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_list_contacts (
  id          INTEGER PRIMARY KEY,
  list_id     INTEGER NOT NULL REFERENCES sync_lists(id) ON DELETE CASCADE,
  entity_type TEXT    NOT NULL,
  entity_id   INTEGER NOT NULL,
  standing    INTEGER NOT NULL CHECK (standing IN (-10, -5, 0, 5, 10)),
  created_at  TEXT    NOT NULL,
  updated_at  TEXT    NOT NULL,
  UNIQUE (list_id, entity_type, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_sync_list_contacts_list ON sync_list_contacts(list_id);

CREATE TABLE IF NOT EXISTS sync_list_targets (
  list_id      INTEGER NOT NULL REFERENCES sync_lists(id) ON DELETE CASCADE,
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  created_at   TEXT    NOT NULL,
  PRIMARY KEY (list_id, character_id)
);
CREATE INDEX IF NOT EXISTS idx_sync_list_targets_character ON sync_list_targets(character_id);

CREATE TABLE IF NOT EXISTS sync_pushed_contacts (
  character_id    INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  entity_type     TEXT    NOT NULL,
  entity_id       INTEGER NOT NULL,
  pushed_standing INTEGER NOT NULL,
  created_at      TEXT    NOT NULL,
  updated_at      TEXT    NOT NULL,
  PRIMARY KEY (character_id, entity_type, entity_id)
);
