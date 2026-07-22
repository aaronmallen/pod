CREATE TABLE IF NOT EXISTS character_contracts (
  character_id    INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  contract_id     INTEGER NOT NULL,
  type            TEXT    NOT NULL,
  status          TEXT    NOT NULL,
  issuer_id       INTEGER NOT NULL,
  issuer_name     TEXT,
  assignee_id     INTEGER,
  assignee_name   TEXT,
  acceptor_id     INTEGER,
  acceptor_name   TEXT,
  price           REAL,
  reward          REAL,
  collateral      REAL,
  volume          REAL,
  for_corporation INTEGER NOT NULL DEFAULT 0,
  date_issued     TEXT    NOT NULL,
  date_expired    TEXT,
  date_completed  TEXT,
  PRIMARY KEY (character_id, contract_id)
);
CREATE INDEX IF NOT EXISTS idx_character_contracts_outstanding
  ON character_contracts(character_id)
  WHERE status = 'outstanding';
