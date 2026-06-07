CREATE TABLE IF NOT EXISTS mail_folder_assignment (
  id                 INTEGER NOT NULL PRIMARY KEY,
  character_id       INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  mail_id            INTEGER NOT NULL,
  folder             TEXT    NOT NULL CHECK(folder IN ('archive', 'trash')),
  remap_label_id     INTEGER,
  soft_delete_intent INTEGER NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_mail_folder_assignment_character_id_mail_id ON mail_folder_assignment(character_id, mail_id);
CREATE INDEX IF NOT EXISTS idx_mail_folder_assignment_character_id ON mail_folder_assignment(character_id);
