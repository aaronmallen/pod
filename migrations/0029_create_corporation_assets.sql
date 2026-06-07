CREATE TABLE IF NOT EXISTS corporation_assets (
  item_id           INTEGER NOT NULL PRIMARY KEY,
  corporation_id    INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  type_id           INTEGER NOT NULL,
  location_id       INTEGER NOT NULL,
  location_type     TEXT    NOT NULL,
  location_flag     TEXT    NOT NULL,
  quantity          INTEGER NOT NULL,
  is_singleton      INTEGER NOT NULL DEFAULT 0,
  is_blueprint_copy INTEGER,
  container_id      INTEGER,
  depth             INTEGER NOT NULL DEFAULT 0,
  is_container      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_corporation_assets_corporation_id         ON corporation_assets(corporation_id);
CREATE INDEX IF NOT EXISTS idx_corporation_assets_owner_container        ON corporation_assets(corporation_id, container_id);
CREATE INDEX IF NOT EXISTS idx_corporation_assets_container             ON corporation_assets(container_id);
CREATE INDEX IF NOT EXISTS idx_corporation_assets_owner_quantity_item   ON corporation_assets(corporation_id, quantity, item_id);
