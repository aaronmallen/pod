CREATE TABLE IF NOT EXISTS character_assets (
  item_id           INTEGER NOT NULL PRIMARY KEY,
  character_id      INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  type_id           INTEGER NOT NULL,
  location_id       INTEGER NOT NULL,
  location_type     TEXT    NOT NULL,
  location_flag     TEXT    NOT NULL,
  quantity          INTEGER NOT NULL,
  is_singleton      INTEGER NOT NULL DEFAULT 0,
  is_blueprint_copy INTEGER,
  is_active_ship    INTEGER NOT NULL DEFAULT 0,
  ship_name         TEXT,
  container_id      INTEGER,
  depth             INTEGER NOT NULL DEFAULT 0,
  is_container      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_character_assets_character_id           ON character_assets(character_id);
CREATE INDEX IF NOT EXISTS idx_character_assets_owner_container        ON character_assets(character_id, container_id);
CREATE INDEX IF NOT EXISTS idx_character_assets_container             ON character_assets(container_id);
CREATE INDEX IF NOT EXISTS idx_character_assets_owner_quantity_item   ON character_assets(character_id, quantity, item_id);
