CREATE TABLE IF NOT EXISTS corporation_abyssal_items (
  item_id           INTEGER NOT NULL PRIMARY KEY,
  corporation_id    INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  type_id           INTEGER NOT NULL,
  source_type_id    INTEGER NOT NULL,
  mutator_type_id   INTEGER NOT NULL,
  dogma_attributes  TEXT    NOT NULL DEFAULT '[]',
  synced_at         INTEGER NOT NULL,
  muta_price_isk    REAL,
  muta_price_synced INTEGER
);
CREATE INDEX IF NOT EXISTS idx_corporation_abyssal_items_corporation_id ON corporation_abyssal_items(corporation_id);
