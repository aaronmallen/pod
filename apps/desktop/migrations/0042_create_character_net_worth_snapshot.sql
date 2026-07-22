CREATE TABLE IF NOT EXISTS character_net_worth_snapshot (
  id           INTEGER NOT NULL PRIMARY KEY,
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  date         TEXT    NOT NULL,
  liquid       REAL    NOT NULL,
  asset_value  REAL,
  escrow       REAL,
  net_worth    REAL    NOT NULL,
  UNIQUE (character_id, date)
);
CREATE INDEX IF NOT EXISTS idx_character_net_worth_snapshot_date ON character_net_worth_snapshot(date);
