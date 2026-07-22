CREATE TABLE IF NOT EXISTS observed_market_orders (
  order_id       INTEGER NOT NULL PRIMARY KEY,
  owner_id       INTEGER NOT NULL,
  is_corporation INTEGER NOT NULL,
  type_id        INTEGER NOT NULL,
  location_id    INTEGER NOT NULL,
  region_id      INTEGER NOT NULL,
  price          REAL    NOT NULL,
  is_buy_order   INTEGER NOT NULL,
  issued         TEXT    NOT NULL,
  first_seen     TEXT    NOT NULL,
  last_seen      TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_observed_market_orders_owner ON observed_market_orders(owner_id, is_corporation, is_buy_order);
