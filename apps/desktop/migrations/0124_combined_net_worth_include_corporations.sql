-- Fold owned-corporation wallet balances into the combined net-worth series.
--
-- character_net_worth_snapshot_combined (0058) summed character snapshots only, so moving ISK
-- between a personal wallet and an owned corp wallet on the same day read as a phantom net-worth
-- swing. corporation_net_worth_snapshot (0043) already stores each owned corp's daily liquid
-- balance, so union those rows in: a corp contributes its liquid net_worth, with asset_value/escrow
-- as 0/NULL (corp assets and contracts are out of scope). The per-date SUM then covers both series.
--
-- Redefining an existing view means DROP + CREATE in a new migration; 0058 stays untouched because
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
  SELECT date, liquid, 0 AS asset_value, NULL AS escrow, net_worth
  FROM corporation_net_worth_snapshot
)
GROUP BY date;
