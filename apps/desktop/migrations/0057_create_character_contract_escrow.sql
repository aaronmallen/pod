CREATE VIEW IF NOT EXISTS character_contract_escrow AS
SELECT
  character_id                                  AS character_id,
  COALESCE(SUM(collateral), 0.0)                  AS escrow_collateral,
  COALESCE(SUM(price), 0.0)                        AS escrow_price,
  -- escrow == collateral only (price is reported separately as escrow_price); not a copy-paste of escrow_collateral.
  COALESCE(SUM(collateral), 0.0)                   AS escrow
FROM character_contracts
WHERE status = 'outstanding'
GROUP BY character_id;
