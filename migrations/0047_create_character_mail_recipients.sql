CREATE TABLE IF NOT EXISTS character_mail_recipients (
  character_id   INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  mail_id        INTEGER NOT NULL,
  recipient_id   INTEGER NOT NULL,
  recipient_type TEXT    NOT NULL CHECK(recipient_type IN ('character', 'corporation', 'alliance', 'mailing_list')),
  recipient_name TEXT    NOT NULL,
  PRIMARY KEY (character_id, mail_id, recipient_id, recipient_type)
);
CREATE INDEX IF NOT EXISTS idx_character_mail_recipients_character_id         ON character_mail_recipients(character_id);
CREATE INDEX IF NOT EXISTS idx_character_mail_recipients_character_id_mail_id ON character_mail_recipients(character_id, mail_id);
