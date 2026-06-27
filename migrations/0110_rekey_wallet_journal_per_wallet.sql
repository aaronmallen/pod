-- Re-key the four wallet ledger tables onto a per-wallet composite identity (ADR-0040).
--
-- EVE reuses one journal `id` / transaction `transaction_id` across both wallets of an
-- internal transfer (one leg per wallet, same id, opposite amount). The tables were keyed
-- on the bare EVE id with `ON CONFLICT(id) DO NOTHING`, so when both legs land in the same
-- table (two divisions of one corporation, or two owned characters) the second leg collided
-- on insert and was silently dropped, overstating budget income/spend. Identity must become
-- per-wallet so both legs persist:
--
--   character_wallet_journal       PK -> (character_id, id)
--   character_wallet_transaction   PK -> (character_id, transaction_id)
--   corporation_wallet_journal     PK -> (corporation_id, division, id)
--   corporation_wallet_transaction PK -> (corporation_id, division, transaction_id)
--
-- SQLite cannot alter a primary key in place, so each table is rebuilt: a new table with the
-- composite key, an `INSERT OR IGNORE ... SELECT` copy of every existing row (loss-free; the
-- old bare-id key already guaranteed uniqueness, and `OR IGNORE` tolerates any historical
-- duplicate the old key would have rejected anyway), a drop of the old table, a rename of the
-- new one into place, and a recreation of every secondary index from migrations 0034/0035/
-- 0040/0041/0088. The EVE id columns are retained unchanged so budget assignments, which key
-- on `(owner, entry_id)` where `entry_id` is exactly this id, keep resolving.
--
-- sqlx runs each migration inside one transaction, so a crash mid-rebuild rolls back to the
-- pre-migration tables. The migration is additive (new file 0110) and never touches the bytes
-- of any released migration, so it cannot trip the sqlx embedded-checksum guard.

-- The `character_state` (0053) and `character_wallet_period_summary` (0059) views read
-- `character_wallet_journal`, and `character_financials` (0061, recreated by 0097) reads
-- `character_state`. SQLite leaves a view dangling when its backing table is dropped and then
-- errors when the next DDL statement re-validates the schema, so drop the whole dependent
-- chain before the rebuild and recreate it verbatim afterward (financials first, since it
-- depends on character_state).
DROP VIEW IF EXISTS character_financials;
DROP VIEW IF EXISTS character_state;
DROP VIEW IF EXISTS character_wallet_period_summary;

-- ---------------------------------------------------------------------------
-- character_wallet_journal -> PRIMARY KEY (character_id, id)
-- ---------------------------------------------------------------------------
CREATE TABLE character_wallet_journal_new (
  id              INTEGER NOT NULL,
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
  tax_receiver_id INTEGER,
  PRIMARY KEY (character_id, id)
);

INSERT OR IGNORE INTO character_wallet_journal_new
  (id, character_id, date, description, ref_type, amount, balance, context_id,
    context_id_type, first_party_id, reason, second_party_id, tax, tax_receiver_id)
SELECT
  id, character_id, date, description, ref_type, amount, balance, context_id,
  context_id_type, first_party_id, reason, second_party_id, tax, tax_receiver_id
FROM character_wallet_journal;

DROP TABLE character_wallet_journal;
ALTER TABLE character_wallet_journal_new RENAME TO character_wallet_journal;

CREATE INDEX IF NOT EXISTS idx_character_wallet_journal_character_id    ON character_wallet_journal(character_id);
CREATE INDEX IF NOT EXISTS idx_character_wallet_journal_character_id_id ON character_wallet_journal(character_id, id);

-- ---------------------------------------------------------------------------
-- character_wallet_transaction -> PRIMARY KEY (character_id, transaction_id)
-- ---------------------------------------------------------------------------
CREATE TABLE character_wallet_transaction_new (
  transaction_id INTEGER NOT NULL,
  character_id   INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  client_id      INTEGER NOT NULL,
  date           TEXT    NOT NULL,
  is_buy         INTEGER NOT NULL,
  is_personal    INTEGER NOT NULL,
  journal_ref_id INTEGER NOT NULL,
  location_id    INTEGER NOT NULL,
  quantity       INTEGER NOT NULL,
  type_id        INTEGER NOT NULL,
  unit_price     REAL    NOT NULL,
  PRIMARY KEY (character_id, transaction_id)
);

INSERT OR IGNORE INTO character_wallet_transaction_new
  (transaction_id, character_id, client_id, date, is_buy, is_personal, journal_ref_id,
    location_id, quantity, type_id, unit_price)
SELECT
  transaction_id, character_id, client_id, date, is_buy, is_personal, journal_ref_id,
  location_id, quantity, type_id, unit_price
FROM character_wallet_transaction;

DROP TABLE character_wallet_transaction;
ALTER TABLE character_wallet_transaction_new RENAME TO character_wallet_transaction;

CREATE INDEX IF NOT EXISTS idx_character_wallet_transaction_character_id   ON character_wallet_transaction(character_id);
CREATE INDEX IF NOT EXISTS idx_character_wallet_transaction_type_id        ON character_wallet_transaction(type_id);
CREATE INDEX IF NOT EXISTS idx_character_wallet_transaction_journal_ref_id ON character_wallet_transaction(journal_ref_id);
CREATE INDEX IF NOT EXISTS idx_character_wallet_transaction_char_tx        ON character_wallet_transaction(character_id, transaction_id);

-- ---------------------------------------------------------------------------
-- corporation_wallet_journal -> PRIMARY KEY (corporation_id, division, id)
-- ---------------------------------------------------------------------------
CREATE TABLE corporation_wallet_journal_new (
  id              INTEGER NOT NULL,
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
  tax_receiver_id INTEGER,
  PRIMARY KEY (corporation_id, division, id)
);

INSERT OR IGNORE INTO corporation_wallet_journal_new
  (id, corporation_id, division, date, description, ref_type, amount, balance, context_id,
    context_id_type, first_party_id, reason, second_party_id, tax, tax_receiver_id)
SELECT
  id, corporation_id, division, date, description, ref_type, amount, balance, context_id,
  context_id_type, first_party_id, reason, second_party_id, tax, tax_receiver_id
FROM corporation_wallet_journal;

DROP TABLE corporation_wallet_journal;
ALTER TABLE corporation_wallet_journal_new RENAME TO corporation_wallet_journal;

CREATE INDEX IF NOT EXISTS idx_corporation_wallet_journal_corporation_id          ON corporation_wallet_journal(corporation_id);
CREATE INDEX IF NOT EXISTS idx_corporation_wallet_journal_corporation_id_division ON corporation_wallet_journal(corporation_id, division);

-- ---------------------------------------------------------------------------
-- corporation_wallet_transaction -> PRIMARY KEY (corporation_id, division, transaction_id)
-- ---------------------------------------------------------------------------
CREATE TABLE corporation_wallet_transaction_new (
  transaction_id INTEGER NOT NULL,
  corporation_id INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  division       INTEGER NOT NULL,
  client_id      INTEGER NOT NULL,
  date           TEXT    NOT NULL,
  is_buy         INTEGER NOT NULL,
  journal_ref_id INTEGER NOT NULL,
  location_id    INTEGER NOT NULL,
  quantity       INTEGER NOT NULL,
  type_id        INTEGER NOT NULL,
  unit_price     REAL    NOT NULL,
  PRIMARY KEY (corporation_id, division, transaction_id)
);

INSERT OR IGNORE INTO corporation_wallet_transaction_new
  (transaction_id, corporation_id, division, client_id, date, is_buy, journal_ref_id,
    location_id, quantity, type_id, unit_price)
SELECT
  transaction_id, corporation_id, division, client_id, date, is_buy, journal_ref_id,
  location_id, quantity, type_id, unit_price
FROM corporation_wallet_transaction;

DROP TABLE corporation_wallet_transaction;
ALTER TABLE corporation_wallet_transaction_new RENAME TO corporation_wallet_transaction;

CREATE INDEX IF NOT EXISTS idx_corporation_wallet_transaction_corporation_id          ON corporation_wallet_transaction(corporation_id);
CREATE INDEX IF NOT EXISTS idx_corporation_wallet_transaction_corporation_id_division ON corporation_wallet_transaction(corporation_id, division);
CREATE INDEX IF NOT EXISTS idx_corporation_wallet_transaction_type_id                 ON corporation_wallet_transaction(type_id);
CREATE INDEX IF NOT EXISTS idx_corporation_wallet_transaction_journal_ref_id          ON corporation_wallet_transaction(journal_ref_id);

-- ---------------------------------------------------------------------------
-- Force a one-time re-fetch of the wallet journals/transactions (ADR-0040 section 4).
--
-- The re-key restores the ability to store both transfer legs, but legs dropped before this
-- migration are simply absent. Deleting the wallet sync-ledger rows makes those jobs present
-- as never-attempted, so the next sync pass re-fetches the full (paginated) history and
-- re-appends it through the now-per-wallet upserts. The upserts are `DO NOTHING`, so the
-- re-fetch is a safe no-op for rows that already exist. A migration runs once per database,
-- so this fires exactly once on upgrade (the one-time-repair precedent of 0102 / 0104).
-- ---------------------------------------------------------------------------
DELETE FROM sync_ledger WHERE kind IN ('CharacterWallet', 'CorporationWallet');

-- ---------------------------------------------------------------------------
-- Recreate the views dropped above the rebuild, verbatim from migrations 0053 and 0059.
-- ---------------------------------------------------------------------------
CREATE VIEW IF NOT EXISTS character_state AS
SELECT
  c.id              AS character_id,
  t.online          AS online,
  t.solar_system_id AS solar_system_id,
  t.station_id      AS station_id,
  t.structure_id    AS structure_id,
  t.ship_item_id    AS ship_item_id,
  t.ship_name       AS ship_name,
  t.ship_type_id    AS ship_type_id,
  t.synced_at       AS synced_at,
  (SELECT SUM(s.skillpoints_in_skill)
    FROM character_skills s
    WHERE s.character_id = c.id)               AS total_sp,
  (
    SELECT anchor.balance + COALESCE((
      SELECT SUM(COALESCE(later.amount, 0))
        FROM character_wallet_journal later
        WHERE later.character_id = anchor.character_id AND later.id > anchor.id
    ), 0)
      FROM character_wallet_journal anchor
      WHERE anchor.character_id = c.id AND anchor.balance IS NOT NULL
      ORDER BY anchor.id DESC
      LIMIT 1
  )                                            AS wallet_balance
FROM characters c
LEFT JOIN character_telemetry t ON t.character_id = c.id;

CREATE VIEW IF NOT EXISTS character_financials AS
SELECT
  c.id AS character_id,
  cs.wallet_balance AS liquid,
  (
    SELECT SUM(a.quantity * CASE WHEN a.is_blueprint_copy = 1 THEN 0
      ELSE COALESCE(ab.muta_price_isk, mp.adjusted_price, mp.average_price, 0) END)
      FROM character_assets a
      LEFT JOIN market_prices mp ON mp.type_id = a.type_id
      LEFT JOIN abyssal_items ab ON ab.item_id = a.item_id
      WHERE a.character_id = c.id
  ) AS asset_value,
  CASE
    WHEN (SELECT SUM(o.escrow) FROM market_orders o WHERE o.character_id = c.id AND o.state = 'open') IS NULL
      AND (SELECT cce.escrow FROM character_contract_escrow cce WHERE cce.character_id = c.id) IS NULL
      THEN NULL
    ELSE
      COALESCE((SELECT SUM(o.escrow) FROM market_orders o WHERE o.character_id = c.id AND o.state = 'open'), 0)
      + COALESCE((SELECT cce.escrow FROM character_contract_escrow cce WHERE cce.character_id = c.id), 0)
  END AS escrow,
  CASE
    WHEN cs.wallet_balance IS NULL
      AND (
        SELECT SUM(a.quantity * CASE WHEN a.is_blueprint_copy = 1 THEN 0
          ELSE COALESCE(ab.muta_price_isk, mp.adjusted_price, mp.average_price, 0) END)
          FROM character_assets a
          LEFT JOIN market_prices mp ON mp.type_id = a.type_id
          LEFT JOIN abyssal_items ab ON ab.item_id = a.item_id
          WHERE a.character_id = c.id
      ) IS NULL
      AND (SELECT SUM(o.escrow) FROM market_orders o WHERE o.character_id = c.id AND o.state = 'open') IS NULL
      AND (SELECT cce.escrow FROM character_contract_escrow cce WHERE cce.character_id = c.id) IS NULL
      THEN NULL
    ELSE
      COALESCE(cs.wallet_balance, 0)
      + COALESCE((
          SELECT SUM(a.quantity * CASE WHEN a.is_blueprint_copy = 1 THEN 0
            ELSE COALESCE(ab.muta_price_isk, mp.adjusted_price, mp.average_price, 0) END)
            FROM character_assets a
            LEFT JOIN market_prices mp ON mp.type_id = a.type_id
            LEFT JOIN abyssal_items ab ON ab.item_id = a.item_id
            WHERE a.character_id = c.id
        ), 0)
      + COALESCE((SELECT SUM(o.escrow) FROM market_orders o WHERE o.character_id = c.id AND o.state = 'open'), 0)
      + COALESCE((SELECT cce.escrow FROM character_contract_escrow cce WHERE cce.character_id = c.id), 0)
  END AS net_worth
FROM characters c
LEFT JOIN character_state cs ON cs.character_id = c.id;

CREATE VIEW IF NOT EXISTS character_wallet_period_summary AS
SELECT
  character_id,
  substr(date, 1, 7)                                          AS period,
  SUM(CASE WHEN amount >= 0 THEN amount ELSE 0.0 END)         AS income,
  SUM(CASE WHEN amount <  0 THEN -amount ELSE 0.0 END)        AS spend,
  SUM(CASE WHEN amount >= 0 THEN amount ELSE 0.0 END)
    - SUM(CASE WHEN amount < 0 THEN -amount ELSE 0.0 END)     AS net
FROM character_wallet_journal
WHERE date IS NOT NULL AND length(date) >= 7
GROUP BY character_id, substr(date, 1, 7);
