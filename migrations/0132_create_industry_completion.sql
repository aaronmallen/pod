CREATE TABLE IF NOT EXISTS industry_completion (
  id              INTEGER NOT NULL PRIMARY KEY,
  character_id    INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  job_id          INTEGER NOT NULL,
  activity_id     INTEGER NOT NULL,
  product_type_id INTEGER,
  runs            INTEGER NOT NULL,
  completed_at    TEXT    NOT NULL,
  created_at      TEXT    NOT NULL,
  UNIQUE (character_id, job_id)
);

CREATE INDEX IF NOT EXISTS idx_industry_completion_character ON industry_completion(character_id);
CREATE INDEX IF NOT EXISTS idx_industry_completion_completed_at ON industry_completion(completed_at);
