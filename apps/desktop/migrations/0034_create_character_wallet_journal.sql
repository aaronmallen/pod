CREATE TABLE IF NOT EXISTS character_wallet_journal (
  id              INTEGER NOT NULL PRIMARY KEY,
  character_id    INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  date            TEXT    NOT NULL,
  description     TEXT    NOT NULL,
  ref_type        TEXT    NOT NULL,
  amount          REAL,
  balance         REAL,
  context_id      INTEGER,
  context_id_type TEXT,
  first_party_id  INTEGER,
  reason          TEXT,
  second_party_id INTEGER,
  tax             REAL,
  tax_receiver_id INTEGER
);
CREATE INDEX IF NOT EXISTS idx_character_wallet_journal_character_id    ON character_wallet_journal(character_id);
CREATE INDEX IF NOT EXISTS idx_character_wallet_journal_character_id_id ON character_wallet_journal(character_id, id);
