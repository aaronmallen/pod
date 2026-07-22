CREATE TABLE IF NOT EXISTS character_mail_body (
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  mail_id      INTEGER NOT NULL,
  body         TEXT    NOT NULL,
  PRIMARY KEY (character_id, mail_id)
);
CREATE INDEX IF NOT EXISTS idx_character_mail_body_character_id ON character_mail_body(character_id);
