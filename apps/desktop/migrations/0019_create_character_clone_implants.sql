CREATE TABLE IF NOT EXISTS character_clone_implants (
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  clone_id     INTEGER,
  type_id      INTEGER NOT NULL,
  name         TEXT    NOT NULL,
  icon         TEXT,
  PRIMARY KEY (character_id, clone_id, type_id)
);
CREATE INDEX IF NOT EXISTS idx_character_clone_implants_character_id ON character_clone_implants(character_id);
CREATE INDEX IF NOT EXISTS idx_character_clone_implants_clone        ON character_clone_implants(character_id, clone_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_character_clone_implants_active
  ON character_clone_implants(character_id, type_id)
  WHERE clone_id IS NULL;
