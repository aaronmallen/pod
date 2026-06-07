CREATE TABLE IF NOT EXISTS character_mail_label_membership (
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  mail_id      INTEGER NOT NULL,
  label_id     INTEGER NOT NULL,
  PRIMARY KEY (character_id, mail_id, label_id),
  FOREIGN KEY (character_id, label_id) REFERENCES character_mail_labels(character_id, label_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_character_mail_label_membership_character_id          ON character_mail_label_membership(character_id);
CREATE INDEX IF NOT EXISTS idx_character_mail_label_membership_character_id_label_id ON character_mail_label_membership(character_id, label_id);
CREATE INDEX IF NOT EXISTS idx_character_mail_label_membership_character_id_mail_id  ON character_mail_label_membership(character_id, mail_id);
