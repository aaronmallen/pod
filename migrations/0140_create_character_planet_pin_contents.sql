CREATE TABLE IF NOT EXISTS character_planet_pin_contents (
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  pin_id       INTEGER NOT NULL,
  type_id      INTEGER NOT NULL,
  amount       INTEGER NOT NULL,
  PRIMARY KEY (character_id, pin_id, type_id)
);
CREATE INDEX IF NOT EXISTS idx_character_planet_pin_contents_character_id    ON character_planet_pin_contents(character_id);
CREATE INDEX IF NOT EXISTS idx_character_planet_pin_contents_owner_pin_id    ON character_planet_pin_contents(character_id, pin_id);
