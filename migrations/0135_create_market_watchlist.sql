CREATE TABLE IF NOT EXISTS market_watchlist (
  id           INTEGER NOT NULL PRIMARY KEY,
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  type_id      INTEGER NOT NULL,
  location_id  INTEGER,
  region_id    INTEGER,
  direction    TEXT    NOT NULL CHECK (direction IN ('buy', 'sell')),
  target_price REAL,
  created_at   TEXT    NOT NULL,
  updated_at   TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_market_watchlist_character ON market_watchlist(character_id);
