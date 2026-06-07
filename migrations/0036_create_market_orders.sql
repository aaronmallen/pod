CREATE TABLE IF NOT EXISTS market_orders (
  order_id      INTEGER NOT NULL PRIMARY KEY,
  character_id  INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  type_id       INTEGER NOT NULL,
  region_id     INTEGER NOT NULL,
  location_id   INTEGER NOT NULL,
  is_buy_order  INTEGER NOT NULL,
  price         REAL    NOT NULL,
  volume_remain INTEGER NOT NULL,
  volume_total  INTEGER NOT NULL,
  escrow        REAL    NOT NULL,
  range         TEXT    NOT NULL,
  duration      INTEGER NOT NULL,
  issued        TEXT    NOT NULL,
  state         TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_market_orders_character_id       ON market_orders(character_id);
CREATE INDEX IF NOT EXISTS idx_market_orders_character_id_state ON market_orders(character_id, state);
