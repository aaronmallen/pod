CREATE TABLE IF NOT EXISTS corporation_net_worth_snapshot (
  id             INTEGER NOT NULL PRIMARY KEY,
  corporation_id INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  date           TEXT    NOT NULL,
  liquid         REAL    NOT NULL,
  net_worth      REAL    NOT NULL,
  UNIQUE (corporation_id, date)
);
CREATE INDEX IF NOT EXISTS idx_corporation_net_worth_snapshot_date ON corporation_net_worth_snapshot(date);
