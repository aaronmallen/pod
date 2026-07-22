CREATE TABLE IF NOT EXISTS character_wallet_transaction (
  transaction_id INTEGER NOT NULL PRIMARY KEY,
  character_id   INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  client_id      INTEGER NOT NULL,
  date           TEXT    NOT NULL,
  is_buy         INTEGER NOT NULL,
  is_personal    INTEGER NOT NULL,
  journal_ref_id INTEGER NOT NULL,
  location_id    INTEGER NOT NULL,
  quantity       INTEGER NOT NULL,
  type_id        INTEGER NOT NULL,
  unit_price     REAL    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_character_wallet_transaction_character_id   ON character_wallet_transaction(character_id);
CREATE INDEX IF NOT EXISTS idx_character_wallet_transaction_type_id        ON character_wallet_transaction(type_id);
CREATE INDEX IF NOT EXISTS idx_character_wallet_transaction_journal_ref_id ON character_wallet_transaction(journal_ref_id);
