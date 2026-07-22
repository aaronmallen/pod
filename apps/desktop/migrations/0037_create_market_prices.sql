CREATE TABLE IF NOT EXISTS market_prices (
  type_id        INTEGER NOT NULL PRIMARY KEY,
  adjusted_price REAL,
  average_price  REAL
);
CREATE INDEX IF NOT EXISTS idx_market_prices_type_id ON market_prices(type_id);
