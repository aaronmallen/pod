CREATE TABLE IF NOT EXISTS tags (
  id          INTEGER PRIMARY KEY NOT NULL,
  color       TEXT,
  created_at  INTEGER NOT NULL,
  description TEXT,
  name        TEXT    NOT NULL,
  position    INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS entity_tags (
  tag_id      INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  entity_type TEXT    NOT NULL,
  entity_id   INTEGER NOT NULL,
  PRIMARY KEY (tag_id, entity_type, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_entity_tags_entity ON entity_tags(entity_type, entity_id);
