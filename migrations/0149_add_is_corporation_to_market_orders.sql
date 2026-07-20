-- ESI's /characters/{id}/orders/ flags corp-placed orders with `is_corporation`; persist it so the
-- same in-game order can be de-duped against the corp-sync copy and excluded from pilot escrow.
ALTER TABLE market_orders ADD COLUMN is_corporation INTEGER NOT NULL DEFAULT 0;
