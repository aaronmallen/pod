ALTER TABLE market_prices ADD COLUMN source TEXT NOT NULL DEFAULT 'esi';
ALTER TABLE market_prices ADD COLUMN fetched_at TEXT;
UPDATE market_prices SET source = 'esi' WHERE source IS NULL;
CREATE INDEX IF NOT EXISTS idx_market_prices_source ON market_prices(source);
