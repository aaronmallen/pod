CREATE TABLE IF NOT EXISTS customs_offices (
  office_id                   INTEGER PRIMARY KEY NOT NULL,
  corporation_id              INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  system_id                   INTEGER NOT NULL REFERENCES solar_systems(id) ON DELETE CASCADE,
  planet_id                   INTEGER,
  standing_level              TEXT    NOT NULL,
  reinforce_exit_start        INTEGER NOT NULL,
  reinforce_exit_end          INTEGER NOT NULL,
  allow_alliance_access       INTEGER NOT NULL DEFAULT 0,
  allow_access_with_standings INTEGER NOT NULL DEFAULT 0,
  alliance_tax_rate           REAL,
  corporation_tax_rate        REAL,
  excellent_standing_tax_rate REAL,
  good_standing_tax_rate      REAL,
  neutral_standing_tax_rate   REAL,
  bad_standing_tax_rate       REAL,
  terrible_standing_tax_rate  REAL,
  synced_at                   TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_customs_offices_corporation_id ON customs_offices(corporation_id);

CREATE INDEX IF NOT EXISTS idx_customs_offices_synced_at ON customs_offices(synced_at);
