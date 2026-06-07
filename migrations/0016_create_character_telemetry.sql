CREATE TABLE IF NOT EXISTS character_telemetry (
  character_id    INTEGER NOT NULL PRIMARY KEY REFERENCES characters(id) ON DELETE CASCADE,
  online          INTEGER NOT NULL,
  solar_system_id INTEGER NOT NULL,
  synced_at       INTEGER NOT NULL,
  ship_item_id    INTEGER,
  ship_name       TEXT,
  ship_type_id    INTEGER,
  station_id      INTEGER,
  structure_id    INTEGER
);
CREATE INDEX IF NOT EXISTS idx_character_telemetry_solar_system_id ON character_telemetry(solar_system_id);
CREATE INDEX IF NOT EXISTS idx_character_telemetry_station_id      ON character_telemetry(station_id);
CREATE INDEX IF NOT EXISTS idx_character_telemetry_structure_id    ON character_telemetry(structure_id);
CREATE INDEX IF NOT EXISTS idx_character_telemetry_ship_type_id    ON character_telemetry(ship_type_id);
