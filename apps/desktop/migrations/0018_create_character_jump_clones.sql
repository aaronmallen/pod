CREATE TABLE IF NOT EXISTS character_jump_clones (
  character_id  INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  jump_clone_id INTEGER NOT NULL,
  location_id   INTEGER NOT NULL,
  location_type TEXT    NOT NULL CHECK(location_type IN ('station', 'structure')),
  location_name TEXT,
  name          TEXT,
  PRIMARY KEY (character_id, jump_clone_id)
);
CREATE INDEX IF NOT EXISTS idx_character_jump_clones_character_id ON character_jump_clones(character_id);
