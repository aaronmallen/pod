CREATE TABLE IF NOT EXISTS character_contact_labels (
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  label_id     INTEGER NOT NULL,
  label_name   TEXT    NOT NULL,
  PRIMARY KEY (character_id, label_id)
);
CREATE INDEX IF NOT EXISTS idx_character_contact_labels_character_id ON character_contact_labels(character_id);
