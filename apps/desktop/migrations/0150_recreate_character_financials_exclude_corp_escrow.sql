-- Recreate character_financials so pilot escrow / net worth sums only the pilot's own open orders.
-- Corp-placed buy-order escrow is ISK locked from the corp wallet, not the pilot's, so `is_corporation`
-- open orders are excluded here; corp net worth is journal-derived and untouched.
DROP VIEW IF EXISTS character_financials;

CREATE VIEW IF NOT EXISTS character_financials AS
SELECT
  c.id AS character_id,
  cs.wallet_balance AS liquid,
  (
    SELECT SUM(a.quantity * CASE WHEN a.is_blueprint_copy = 1 THEN 0
      ELSE COALESCE(ab.muta_price_isk, mp.adjusted_price, mp.average_price, 0) END)
      FROM character_assets a
      LEFT JOIN market_prices mp ON mp.type_id = a.type_id
      LEFT JOIN abyssal_items ab ON ab.item_id = a.item_id
      WHERE a.character_id = c.id
  ) AS asset_value,
  CASE
    WHEN (SELECT SUM(o.escrow) FROM market_orders o WHERE o.character_id = c.id AND o.state = 'open' AND o.is_corporation = 0) IS NULL
      AND (SELECT cce.escrow FROM character_contract_escrow cce WHERE cce.character_id = c.id) IS NULL
      THEN NULL
    ELSE
      COALESCE((SELECT SUM(o.escrow) FROM market_orders o WHERE o.character_id = c.id AND o.state = 'open' AND o.is_corporation = 0), 0)
      + COALESCE((SELECT cce.escrow FROM character_contract_escrow cce WHERE cce.character_id = c.id), 0)
  END AS escrow,
  CASE
    WHEN cs.wallet_balance IS NULL
      AND (
        SELECT SUM(a.quantity * CASE WHEN a.is_blueprint_copy = 1 THEN 0
          ELSE COALESCE(ab.muta_price_isk, mp.adjusted_price, mp.average_price, 0) END)
          FROM character_assets a
          LEFT JOIN market_prices mp ON mp.type_id = a.type_id
          LEFT JOIN abyssal_items ab ON ab.item_id = a.item_id
          WHERE a.character_id = c.id
      ) IS NULL
      AND (SELECT SUM(o.escrow) FROM market_orders o WHERE o.character_id = c.id AND o.state = 'open' AND o.is_corporation = 0) IS NULL
      AND (SELECT cce.escrow FROM character_contract_escrow cce WHERE cce.character_id = c.id) IS NULL
      THEN NULL
    ELSE
      COALESCE(cs.wallet_balance, 0)
      + COALESCE((
          SELECT SUM(a.quantity * CASE WHEN a.is_blueprint_copy = 1 THEN 0
            ELSE COALESCE(ab.muta_price_isk, mp.adjusted_price, mp.average_price, 0) END)
            FROM character_assets a
            LEFT JOIN market_prices mp ON mp.type_id = a.type_id
            LEFT JOIN abyssal_items ab ON ab.item_id = a.item_id
            WHERE a.character_id = c.id
        ), 0)
      + COALESCE((SELECT SUM(o.escrow) FROM market_orders o WHERE o.character_id = c.id AND o.state = 'open' AND o.is_corporation = 0), 0)
      + COALESCE((SELECT cce.escrow FROM character_contract_escrow cce WHERE cce.character_id = c.id), 0)
  END AS net_worth
FROM characters c
LEFT JOIN character_state cs ON cs.character_id = c.id;
