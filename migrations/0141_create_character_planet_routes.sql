CREATE TABLE IF NOT EXISTS character_planet_routes (
  character_id        INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  route_id            INTEGER NOT NULL,
  planet_id           INTEGER NOT NULL,
  content_type_id     INTEGER NOT NULL,
  destination_pin_id  INTEGER NOT NULL,
  quantity            REAL    NOT NULL,
  source_pin_id       INTEGER NOT NULL,
  PRIMARY KEY (character_id, route_id)
);
CREATE INDEX IF NOT EXISTS idx_character_planet_routes_character_id    ON character_planet_routes(character_id);
CREATE INDEX IF NOT EXISTS idx_character_planet_routes_owner_planet_id ON character_planet_routes(character_id, planet_id);
