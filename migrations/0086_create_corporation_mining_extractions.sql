CREATE TABLE IF NOT EXISTS corporation_mining_extractions (
  corporation_id        INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  structure_id          INTEGER NOT NULL,
  moon_id               INTEGER NOT NULL,
  chunk_arrival_time    TEXT,
  extraction_start_time TEXT,
  natural_decay_time    TEXT,
  PRIMARY KEY (corporation_id, structure_id, moon_id)
);
CREATE INDEX IF NOT EXISTS idx_corporation_mining_extractions_corporation_id ON corporation_mining_extractions(corporation_id);
