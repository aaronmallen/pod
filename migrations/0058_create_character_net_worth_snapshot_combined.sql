CREATE VIEW IF NOT EXISTS character_net_worth_snapshot_combined AS
SELECT
  date                AS date,
  SUM(liquid)         AS liquid,
  SUM(asset_value)    AS asset_value,
  SUM(escrow)         AS escrow,
  SUM(net_worth)      AS net_worth
FROM character_net_worth_snapshot
GROUP BY date;
