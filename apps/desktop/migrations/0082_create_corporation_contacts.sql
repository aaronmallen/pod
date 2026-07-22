CREATE TABLE IF NOT EXISTS corporation_contacts (
  corporation_id INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  contact_id     INTEGER NOT NULL,
  contact_type   TEXT    NOT NULL CHECK(contact_type IN ('character', 'corporation', 'alliance', 'faction')),
  standing       REAL    NOT NULL,
  is_watched     INTEGER NOT NULL DEFAULT 0,
  is_blocked     INTEGER NOT NULL DEFAULT 0,
  label_ids      TEXT    NOT NULL DEFAULT '[]',
  contact_name   TEXT    NOT NULL,
  PRIMARY KEY (corporation_id, contact_id)
);
CREATE INDEX IF NOT EXISTS idx_corporation_contacts_corporation_id ON corporation_contacts(corporation_id);
CREATE TABLE IF NOT EXISTS corporation_contact_labels (
  corporation_id INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  label_id       INTEGER NOT NULL,
  label_name     TEXT    NOT NULL,
  PRIMARY KEY (corporation_id, label_id)
);
CREATE INDEX IF NOT EXISTS idx_corporation_contact_labels_corporation_id ON corporation_contact_labels(corporation_id);
