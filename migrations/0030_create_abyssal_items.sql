CREATE TABLE IF NOT EXISTS abyssal_items (
  item_id           INTEGER NOT NULL PRIMARY KEY,
  character_id      INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  type_id           INTEGER NOT NULL,
  source_type_id    INTEGER NOT NULL,
  mutator_type_id   INTEGER NOT NULL,
  dogma_attributes  TEXT    NOT NULL DEFAULT '[]',
  synced_at         INTEGER NOT NULL,
  muta_price_isk    REAL,
  muta_price_synced INTEGER
);
CREATE INDEX IF NOT EXISTS idx_abyssal_items_character_id ON abyssal_items(character_id);
