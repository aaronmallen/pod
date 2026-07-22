CREATE TABLE IF NOT EXISTS corporation_blueprints (
  item_id             INTEGER NOT NULL PRIMARY KEY,
  corporation_id      INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  type_id             INTEGER NOT NULL,
  location_id         INTEGER NOT NULL,
  location_flag       TEXT    NOT NULL,
  quantity            INTEGER NOT NULL,
  material_efficiency INTEGER NOT NULL,
  time_efficiency     INTEGER NOT NULL,
  runs                INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_corporation_blueprints_corporation_id     ON corporation_blueprints(corporation_id);
CREATE INDEX IF NOT EXISTS idx_corporation_blueprints_owner_type_id      ON corporation_blueprints(corporation_id, type_id);
