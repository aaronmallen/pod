CREATE TABLE IF NOT EXISTS mail_drafts (
  id            INTEGER NOT NULL PRIMARY KEY,
  character_id  INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  subject       TEXT    NOT NULL DEFAULT '',
  body          TEXT    NOT NULL DEFAULT '',
  recipients_to TEXT    NOT NULL DEFAULT '[]',
  recipients_cc TEXT    NOT NULL DEFAULT '[]',
  kind          TEXT    NOT NULL DEFAULT 'New',
  quote         TEXT,
  created_at    TEXT    NOT NULL,
  updated_at    TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_mail_drafts_character_id ON mail_drafts(character_id);
