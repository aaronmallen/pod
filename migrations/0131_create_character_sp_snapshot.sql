CREATE TABLE IF NOT EXISTS character_sp_snapshot (
  id             INTEGER NOT NULL PRIMARY KEY,
  character_id   INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  date           TEXT    NOT NULL,
  total_sp       INTEGER NOT NULL,
  unallocated_sp INTEGER NOT NULL DEFAULT 0,
  UNIQUE (character_id, date)
);

CREATE INDEX IF NOT EXISTS idx_character_sp_snapshot_date ON character_sp_snapshot(date);
