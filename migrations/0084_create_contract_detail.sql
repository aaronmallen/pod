-- Contract detail: header columns previously discarded on character_contracts, a corp-keyed
-- corporation_contracts mirror, and the items/bids child tables for both owners. The child tables
-- FK to their owner table (characters(id) / corporations(id)) on the same DELETE CASCADE pattern
-- the killmail detail child tables (0074/0083) use, rather than to the contracts table.
ALTER TABLE character_contracts ADD COLUMN title                 TEXT;
ALTER TABLE character_contracts ADD COLUMN availability          TEXT;
ALTER TABLE character_contracts ADD COLUMN days_to_complete      INTEGER;
ALTER TABLE character_contracts ADD COLUMN start_location_id     INTEGER;
ALTER TABLE character_contracts ADD COLUMN end_location_id       INTEGER;
ALTER TABLE character_contracts ADD COLUMN date_accepted         TEXT;
ALTER TABLE character_contracts ADD COLUMN issuer_corporation_id INTEGER;

CREATE TABLE IF NOT EXISTS corporation_contracts (
  corporation_id        INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  contract_id           INTEGER NOT NULL,
  type                  TEXT    NOT NULL,
  status                TEXT    NOT NULL,
  issuer_id             INTEGER NOT NULL,
  issuer_name           TEXT,
  assignee_id           INTEGER,
  assignee_name         TEXT,
  acceptor_id           INTEGER,
  acceptor_name         TEXT,
  price                 REAL,
  reward                REAL,
  collateral            REAL,
  volume                REAL,
  for_corporation       INTEGER NOT NULL DEFAULT 0,
  date_issued           TEXT    NOT NULL,
  date_expired          TEXT,
  date_completed        TEXT,
  title                 TEXT,
  availability          TEXT,
  days_to_complete      INTEGER,
  start_location_id     INTEGER,
  end_location_id       INTEGER,
  date_accepted         TEXT,
  issuer_corporation_id INTEGER,
  PRIMARY KEY (corporation_id, contract_id)
);
CREATE INDEX IF NOT EXISTS idx_corporation_contracts_corporation_id ON corporation_contracts(corporation_id);

CREATE TABLE IF NOT EXISTS character_contract_items (
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  contract_id  INTEGER NOT NULL,
  record_id    INTEGER NOT NULL,
  type_id      INTEGER NOT NULL,
  quantity     INTEGER NOT NULL DEFAULT 0,
  raw_quantity INTEGER,
  is_singleton INTEGER NOT NULL DEFAULT 0,
  is_included  INTEGER NOT NULL DEFAULT 0,
  value_isk    REAL    NOT NULL DEFAULT 0,
  PRIMARY KEY (character_id, contract_id, record_id)
);
CREATE INDEX IF NOT EXISTS idx_character_contract_items_character_id_contract_id ON character_contract_items(character_id, contract_id);

CREATE TABLE IF NOT EXISTS corporation_contract_items (
  corporation_id INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  contract_id    INTEGER NOT NULL,
  record_id      INTEGER NOT NULL,
  type_id        INTEGER NOT NULL,
  quantity       INTEGER NOT NULL DEFAULT 0,
  raw_quantity   INTEGER,
  is_singleton   INTEGER NOT NULL DEFAULT 0,
  is_included    INTEGER NOT NULL DEFAULT 0,
  value_isk      REAL    NOT NULL DEFAULT 0,
  PRIMARY KEY (corporation_id, contract_id, record_id)
);
CREATE INDEX IF NOT EXISTS idx_corporation_contract_items_corporation_id_contract_id ON corporation_contract_items(corporation_id, contract_id);

CREATE TABLE IF NOT EXISTS character_contract_bids (
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  contract_id  INTEGER NOT NULL,
  bid_id       INTEGER NOT NULL,
  bidder_id    INTEGER NOT NULL,
  amount       REAL    NOT NULL DEFAULT 0,
  date_bid     TEXT    NOT NULL,
  PRIMARY KEY (character_id, contract_id, bid_id)
);
CREATE INDEX IF NOT EXISTS idx_character_contract_bids_character_id_contract_id ON character_contract_bids(character_id, contract_id);

CREATE TABLE IF NOT EXISTS corporation_contract_bids (
  corporation_id INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  contract_id    INTEGER NOT NULL,
  bid_id         INTEGER NOT NULL,
  bidder_id      INTEGER NOT NULL,
  amount         REAL    NOT NULL DEFAULT 0,
  date_bid       TEXT    NOT NULL,
  PRIMARY KEY (corporation_id, contract_id, bid_id)
);
CREATE INDEX IF NOT EXISTS idx_corporation_contract_bids_corporation_id_contract_id ON corporation_contract_bids(corporation_id, contract_id);
