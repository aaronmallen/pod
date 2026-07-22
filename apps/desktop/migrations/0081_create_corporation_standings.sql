CREATE TABLE IF NOT EXISTS corporation_standings (
  corporation_id INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  from_id        INTEGER NOT NULL,
  from_type      TEXT    NOT NULL CHECK(from_type IN ('faction', 'npc_corp', 'agent')),
  standing       REAL    NOT NULL,
  from_name      TEXT    NOT NULL,
  PRIMARY KEY (corporation_id, from_id)
);
CREATE INDEX IF NOT EXISTS idx_corporation_standings_corporation_id ON corporation_standings(corporation_id);
