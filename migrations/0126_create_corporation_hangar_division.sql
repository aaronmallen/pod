CREATE TABLE IF NOT EXISTS corporation_hangar_division (
  corporation_id INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  division       INTEGER NOT NULL,
  name           TEXT,
  PRIMARY KEY (corporation_id, division)
);
CREATE INDEX IF NOT EXISTS idx_corporation_hangar_division_corporation_id ON corporation_hangar_division(corporation_id);
