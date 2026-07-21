DELETE FROM market_watchlist
WHERE id NOT IN (
  SELECT id FROM (
    SELECT
      id,
      ROW_NUMBER() OVER (
        PARTITION BY type_id, direction, location_id, location_tier, region_id
        ORDER BY updated_at DESC, id DESC
      ) AS pos
    FROM market_watchlist
  )
  WHERE pos = 1
);

DROP INDEX IF EXISTS idx_market_watchlist_character;

CREATE TABLE market_watchlist_global (
  id            INTEGER NOT NULL PRIMARY KEY,
  type_id       INTEGER NOT NULL,
  location_id   INTEGER,
  region_id     INTEGER,
  direction     TEXT    NOT NULL CHECK (direction IN ('buy', 'sell')),
  target_price  REAL,
  created_at    TEXT    NOT NULL,
  updated_at    TEXT    NOT NULL,
  location_tier TEXT,
  sort_order    INTEGER NOT NULL DEFAULT 0
);

INSERT INTO market_watchlist_global
  (id, type_id, location_id, region_id, direction, target_price, created_at, updated_at, location_tier, sort_order)
SELECT id, type_id, location_id, region_id, direction, target_price, created_at, updated_at, location_tier, sort_order
FROM market_watchlist;

DROP TABLE market_watchlist;

ALTER TABLE market_watchlist_global RENAME TO market_watchlist;
