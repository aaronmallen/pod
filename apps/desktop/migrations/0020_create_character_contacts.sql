CREATE TABLE IF NOT EXISTS character_contacts (
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  contact_id   INTEGER NOT NULL,
  contact_type TEXT    NOT NULL CHECK(contact_type IN ('character', 'corporation', 'alliance', 'faction')),
  standing     REAL    NOT NULL,
  is_watched   INTEGER NOT NULL DEFAULT 0,
  is_blocked   INTEGER NOT NULL DEFAULT 0,
  label_ids    TEXT    NOT NULL DEFAULT '[]',
  contact_name TEXT    NOT NULL,
  PRIMARY KEY (character_id, contact_id)
);
CREATE INDEX IF NOT EXISTS idx_character_contacts_character_id ON character_contacts(character_id);
