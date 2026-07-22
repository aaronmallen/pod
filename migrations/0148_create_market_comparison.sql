CREATE TABLE IF NOT EXISTS market_comparison (
  id       INTEGER NOT NULL PRIMARY KEY,
  place_id INTEGER NOT NULL UNIQUE,
  tier     TEXT    NOT NULL
);
