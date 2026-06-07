CREATE TABLE IF NOT EXISTS corporation_wallet_transaction (
  transaction_id INTEGER NOT NULL PRIMARY KEY,
  corporation_id INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  division       INTEGER NOT NULL,
  client_id      INTEGER NOT NULL,
  date           TEXT    NOT NULL,
  is_buy         INTEGER NOT NULL,
  journal_ref_id INTEGER NOT NULL,
  location_id    INTEGER NOT NULL,
  quantity       INTEGER NOT NULL,
  type_id        INTEGER NOT NULL,
  unit_price     REAL    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_corporation_wallet_transaction_corporation_id          ON corporation_wallet_transaction(corporation_id);
CREATE INDEX IF NOT EXISTS idx_corporation_wallet_transaction_corporation_id_division ON corporation_wallet_transaction(corporation_id, division);
CREATE INDEX IF NOT EXISTS idx_corporation_wallet_transaction_type_id                 ON corporation_wallet_transaction(type_id);
CREATE INDEX IF NOT EXISTS idx_corporation_wallet_transaction_journal_ref_id          ON corporation_wallet_transaction(journal_ref_id);
