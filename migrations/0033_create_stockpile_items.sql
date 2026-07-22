CREATE TABLE IF NOT EXISTS stockpile_items (
  id              INTEGER NOT NULL PRIMARY KEY,
  stockpile_id    INTEGER NOT NULL REFERENCES stockpiles(id) ON DELETE CASCADE,
  type_id         INTEGER NOT NULL,
  target_quantity INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_stockpile_items_stockpile_id ON stockpile_items(stockpile_id);
