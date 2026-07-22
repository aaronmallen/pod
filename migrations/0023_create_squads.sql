CREATE TABLE IF NOT EXISTS squads (
  id          INTEGER PRIMARY KEY NOT NULL,
  color       TEXT,
  created_at  INTEGER NOT NULL,
  description TEXT,
  name        TEXT    NOT NULL,
  position    INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS character_squads (
  character_id INTEGER PRIMARY KEY NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  position     INTEGER NOT NULL,
  squad_id     INTEGER NOT NULL REFERENCES squads(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_character_squads_squad_id ON character_squads(squad_id);
