CREATE TABLE IF NOT EXISTS character_skillqueue (
  character_id      INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  queue_position    INTEGER NOT NULL,
  skill_id          INTEGER NOT NULL,
  finished_level    INTEGER NOT NULL,
  finish_date       TEXT,
  level_end_sp      INTEGER,
  level_start_sp    INTEGER,
  start_date        TEXT,
  training_start_sp INTEGER,
  PRIMARY KEY (character_id, queue_position)
);
CREATE INDEX IF NOT EXISTS idx_character_skillqueue_skill_id ON character_skillqueue(skill_id);
