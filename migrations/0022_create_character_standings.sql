CREATE TABLE IF NOT EXISTS character_standings (
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  from_id      INTEGER NOT NULL,
  from_type    TEXT    NOT NULL CHECK(from_type IN ('faction', 'npc_corp', 'agent')),
  standing     REAL    NOT NULL,
  from_name    TEXT    NOT NULL,
  PRIMARY KEY (character_id, from_id)
);
CREATE INDEX IF NOT EXISTS idx_character_standings_character_id ON character_standings(character_id);
