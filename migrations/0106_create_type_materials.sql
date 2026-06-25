CREATE TABLE IF NOT EXISTS type_materials (
  type_id          INTEGER NOT NULL,
  material_type_id INTEGER NOT NULL,
  quantity         INTEGER NOT NULL,
  PRIMARY KEY (type_id, material_type_id)
);
CREATE INDEX IF NOT EXISTS idx_type_materials_type ON type_materials(type_id);
