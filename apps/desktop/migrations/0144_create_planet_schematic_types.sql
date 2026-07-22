CREATE TABLE IF NOT EXISTS planet_schematic_types (
  schematic_id INTEGER NOT NULL,
  type_id      INTEGER NOT NULL,
  is_input     INTEGER NOT NULL,
  quantity     INTEGER NOT NULL,
  PRIMARY KEY (schematic_id, type_id)
);
CREATE INDEX IF NOT EXISTS idx_planet_schematic_types_type ON planet_schematic_types(type_id);
