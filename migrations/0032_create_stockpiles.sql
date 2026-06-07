CREATE TABLE IF NOT EXISTS stockpiles (
  id           INTEGER NOT NULL PRIMARY KEY,
  name         TEXT    NOT NULL,
  character_id INTEGER REFERENCES characters(id) ON DELETE CASCADE,
  location_id  INTEGER
);
CREATE INDEX IF NOT EXISTS idx_stockpiles_character_id ON stockpiles(character_id);
