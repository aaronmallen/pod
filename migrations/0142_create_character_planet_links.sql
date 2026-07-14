CREATE TABLE IF NOT EXISTS character_planet_links (
  character_id       INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  source_pin_id      INTEGER NOT NULL,
  destination_pin_id INTEGER NOT NULL,
  planet_id          INTEGER NOT NULL,
  link_level         INTEGER NOT NULL,
  PRIMARY KEY (character_id, source_pin_id, destination_pin_id)
);
CREATE INDEX IF NOT EXISTS idx_character_planet_links_character_id    ON character_planet_links(character_id);
CREATE INDEX IF NOT EXISTS idx_character_planet_links_owner_planet_id ON character_planet_links(character_id, planet_id);
