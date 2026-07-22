CREATE TABLE IF NOT EXISTS mail_triage (
  id           INTEGER PRIMARY KEY,
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  mail_id      INTEGER NOT NULL,
  star         INTEGER NOT NULL DEFAULT 0,
  pin          INTEGER NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_mail_triage_character_id_mail_id ON mail_triage(character_id, mail_id);
CREATE INDEX IF NOT EXISTS idx_mail_triage_character_id ON mail_triage(character_id);
