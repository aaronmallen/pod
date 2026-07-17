ALTER TABLE market_watchlist ADD COLUMN location_tier TEXT;

-- Back-fill pre-scope rows as region-wide watches: the location becomes the whole region.
UPDATE market_watchlist
SET location_id = region_id,
    location_tier = 'region'
WHERE location_tier IS NULL;
