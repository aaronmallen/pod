CREATE TABLE IF NOT EXISTS character_mail_labels (
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  label_id     INTEGER NOT NULL,
  name         TEXT    NOT NULL,
  color        TEXT,
  PRIMARY KEY (character_id, label_id)
);
CREATE INDEX IF NOT EXISTS idx_character_mail_labels_character_id ON character_mail_labels(character_id);
