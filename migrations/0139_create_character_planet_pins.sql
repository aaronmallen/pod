CREATE TABLE IF NOT EXISTS character_planet_pins (
  character_id     INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  pin_id           INTEGER NOT NULL,
  planet_id        INTEGER NOT NULL,
  cycle_time       INTEGER,
  expiry_time      TEXT,
  head_radius      REAL,
  install_time     TEXT,
  last_cycle_start TEXT,
  latitude         REAL    NOT NULL,
  longitude        REAL    NOT NULL,
  product_type_id  INTEGER,
  qty_per_cycle    INTEGER,
  schematic_id     INTEGER,
  type_id          INTEGER NOT NULL,
  PRIMARY KEY (character_id, pin_id)
);
CREATE INDEX IF NOT EXISTS idx_character_planet_pins_character_id     ON character_planet_pins(character_id);
CREATE INDEX IF NOT EXISTS idx_character_planet_pins_owner_planet_id  ON character_planet_pins(character_id, planet_id);
