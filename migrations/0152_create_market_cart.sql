CREATE TABLE IF NOT EXISTS market_cart (
  id         INTEGER PRIMARY KEY,
  name       TEXT,
  is_live    INTEGER NOT NULL DEFAULT 0,
  created_at TEXT    NOT NULL,
  updated_at TEXT    NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_market_cart_live ON market_cart(is_live) WHERE is_live = 1;

CREATE TABLE IF NOT EXISTS market_cart_line (
  id       INTEGER PRIMARY KEY,
  cart_id  INTEGER NOT NULL REFERENCES market_cart(id) ON DELETE CASCADE,
  type_id  INTEGER NOT NULL,
  quantity INTEGER NOT NULL,
  position INTEGER NOT NULL,
  UNIQUE (cart_id, type_id)
);
CREATE INDEX IF NOT EXISTS idx_market_cart_line_cart ON market_cart_line(cart_id, position);
