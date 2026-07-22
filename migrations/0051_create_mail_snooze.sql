CREATE TABLE IF NOT EXISTS mail_snooze (
  id           INTEGER PRIMARY KEY,
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  mail_id      INTEGER NOT NULL,
  snooze_until TEXT    NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_mail_snooze_character_id_mail_id ON mail_snooze(character_id, mail_id);
CREATE INDEX IF NOT EXISTS idx_mail_snooze_character_id ON mail_snooze(character_id);
CREATE INDEX IF NOT EXISTS idx_mail_snooze_snooze_until ON mail_snooze(snooze_until);
