ALTER TABLE tags ADD COLUMN scope TEXT NOT NULL DEFAULT 'entity';

CREATE TABLE IF NOT EXISTS tag_scope_seeded (
  scope      TEXT NOT NULL,
  seeded_at  TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tag_scope_seeded_unique
  ON tag_scope_seeded(scope);
