CREATE TABLE IF NOT EXISTS corporation_wallet_journal (
  id              INTEGER NOT NULL PRIMARY KEY,
  corporation_id  INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  division        INTEGER NOT NULL,
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
CREATE INDEX IF NOT EXISTS idx_corporation_wallet_journal_corporation_id          ON corporation_wallet_journal(corporation_id);
CREATE INDEX IF NOT EXISTS idx_corporation_wallet_journal_corporation_id_division ON corporation_wallet_journal(corporation_id, division);
