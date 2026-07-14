CREATE TABLE IF NOT EXISTS character_planets (
  character_id    INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  planet_id       INTEGER NOT NULL,
  last_update     TEXT    NOT NULL,
  num_pins        INTEGER NOT NULL,
  planet_type     TEXT    NOT NULL,
  solar_system_id INTEGER NOT NULL,
  upgrade_level   INTEGER NOT NULL,
  PRIMARY KEY (character_id, planet_id)
);
CREATE INDEX IF NOT EXISTS idx_character_planets_character_id ON character_planets(character_id);
