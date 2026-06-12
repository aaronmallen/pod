CREATE TABLE IF NOT EXISTS character_calendar (
  character_id     INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  event_id         INTEGER NOT NULL,
  timestamp        TEXT    NOT NULL,
  duration_minutes INTEGER NOT NULL DEFAULT 0,
  importance       INTEGER NOT NULL DEFAULT 0,
  owner_id         INTEGER NOT NULL,
  owner_name       TEXT    NOT NULL,
  owner_type       TEXT    NOT NULL,
  response         TEXT    NOT NULL,
  title            TEXT    NOT NULL,
  body             TEXT,
  fetched_at       TEXT    NOT NULL,
  PRIMARY KEY (character_id, event_id)
);
CREATE INDEX IF NOT EXISTS idx_character_calendar_character_id           ON character_calendar(character_id);
CREATE INDEX IF NOT EXISTS idx_character_calendar_character_id_timestamp ON character_calendar(character_id, timestamp);
