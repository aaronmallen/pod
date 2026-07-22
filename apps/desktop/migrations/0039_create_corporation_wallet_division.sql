CREATE TABLE IF NOT EXISTS corporation_wallet_division (
  corporation_id INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  division       INTEGER NOT NULL,
  name           TEXT,
  balance        REAL,
  PRIMARY KEY (corporation_id, division)
);
CREATE INDEX IF NOT EXISTS idx_corporation_wallet_division_corporation_id ON corporation_wallet_division(corporation_id);
