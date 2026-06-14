CREATE TABLE IF NOT EXISTS blueprint_activity_materials (
  blueprint_type_id INTEGER NOT NULL,
  activity_id       INTEGER NOT NULL,
  material_type_id  INTEGER NOT NULL,
  quantity          INTEGER NOT NULL,
  PRIMARY KEY (blueprint_type_id, activity_id, material_type_id)
);
CREATE INDEX IF NOT EXISTS idx_blueprint_activity_materials_blueprint_activity ON blueprint_activity_materials(blueprint_type_id, activity_id);
