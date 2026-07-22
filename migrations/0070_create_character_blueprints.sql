CREATE TABLE IF NOT EXISTS character_blueprints (
  item_id             INTEGER NOT NULL PRIMARY KEY,
  character_id        INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  type_id             INTEGER NOT NULL,
  location_id         INTEGER NOT NULL,
  location_flag       TEXT    NOT NULL,
  quantity            INTEGER NOT NULL,
  material_efficiency INTEGER NOT NULL,
  time_efficiency     INTEGER NOT NULL,
  runs                INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_character_blueprints_character_id         ON character_blueprints(character_id);
CREATE INDEX IF NOT EXISTS idx_character_blueprints_owner_type_id        ON character_blueprints(character_id, type_id);
