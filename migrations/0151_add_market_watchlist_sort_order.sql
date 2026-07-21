ALTER TABLE market_watchlist ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;

WITH ordered AS (
  SELECT
    id,
    ROW_NUMBER() OVER (ORDER BY created_at DESC, id DESC) - 1 AS pos
  FROM market_watchlist
)
UPDATE market_watchlist
SET sort_order = (SELECT pos FROM ordered WHERE ordered.id = market_watchlist.id);
