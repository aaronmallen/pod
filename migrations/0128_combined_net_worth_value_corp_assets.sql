-- Fold owned-corporation asset value into the combined net-worth series.
--
-- 0124 unioned each owned corp's snapshot in with a hardcoded 0 AS asset_value, so corp net worth
-- counted liquid cash only and swung whenever a corp moved ISK in or out of assets. Now that
-- corporation_net_worth_snapshot carries asset_value (0127) and record_today writes
-- net_worth = liquid + asset_value, the corporation branch selects COALESCE(asset_value, 0) so
-- SUM(asset_value) and SUM(net_worth) include corp assets. The character branch is unchanged.
--
-- Redefining an existing view means DROP + CREATE in a new migration; 0124 stays untouched because
-- existing databases already applied it.

DROP VIEW IF EXISTS character_net_worth_snapshot_combined;

CREATE VIEW IF NOT EXISTS character_net_worth_snapshot_combined AS
SELECT
  date                AS date,
  SUM(liquid)         AS liquid,
  SUM(asset_value)    AS asset_value,
  SUM(escrow)         AS escrow,
  SUM(net_worth)      AS net_worth
FROM (
  SELECT date, liquid, asset_value, escrow, net_worth
  FROM character_net_worth_snapshot
  UNION ALL
  SELECT date, liquid, COALESCE(asset_value, 0.0) AS asset_value, NULL AS escrow, net_worth
  FROM corporation_net_worth_snapshot
)
GROUP BY date;
