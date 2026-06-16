CREATE TABLE IF NOT EXISTS moons (
  id              INTEGER PRIMARY KEY NOT NULL,
  planet_id       INTEGER,
  solar_system_id INTEGER NOT NULL REFERENCES solar_systems(id),
  name            TEXT    NOT NULL,
  orbit_index     INTEGER,
  position_x      REAL    NOT NULL,
  position_y      REAL    NOT NULL,
  position_z      REAL    NOT NULL,
  radius          REAL,
  type_id         INTEGER
);

CREATE INDEX IF NOT EXISTS idx_moons_solar_system_id ON moons(solar_system_id);
