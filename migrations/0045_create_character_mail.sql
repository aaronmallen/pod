CREATE TABLE IF NOT EXISTS character_mail (
  character_id   INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  mail_id        INTEGER NOT NULL,
  from_id        INTEGER NOT NULL,
  from_name      TEXT    NOT NULL,
  subject        TEXT,
  timestamp      TEXT    NOT NULL,
  is_read        INTEGER NOT NULL DEFAULT 0,
  has_attachment INTEGER NOT NULL DEFAULT 0,
  important      INTEGER NOT NULL DEFAULT 0,
  from_corp      INTEGER NOT NULL DEFAULT 0,
  from_system    INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (character_id, mail_id)
);
CREATE INDEX IF NOT EXISTS idx_character_mail_character_id           ON character_mail(character_id);
CREATE INDEX IF NOT EXISTS idx_character_mail_character_id_timestamp ON character_mail(character_id, timestamp);
