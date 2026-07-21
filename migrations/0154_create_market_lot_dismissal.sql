CREATE TABLE IF NOT EXISTS market_lot_dismissal (
  transaction_id INTEGER NOT NULL,
  owner_id       INTEGER NOT NULL,
  is_corporation INTEGER NOT NULL,
  dismissed_at   TEXT    NOT NULL,
  PRIMARY KEY (transaction_id, owner_id, is_corporation)
);

CREATE INDEX IF NOT EXISTS idx_market_lot_dismissal_owner ON market_lot_dismissal(owner_id);
