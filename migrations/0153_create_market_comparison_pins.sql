CREATE TABLE IF NOT EXISTS market_comparison_pin (
  id       INTEGER NOT NULL PRIMARY KEY,
  type_id  INTEGER NOT NULL,
  position INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS market_comparison_pin_market (
  id       INTEGER NOT NULL PRIMARY KEY,
  pin_id   INTEGER NOT NULL REFERENCES market_comparison_pin(id) ON DELETE CASCADE,
  place_id INTEGER NOT NULL,
  tier     TEXT    NOT NULL,
  position INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_market_comparison_pin_market_pin ON market_comparison_pin_market(pin_id, position);
