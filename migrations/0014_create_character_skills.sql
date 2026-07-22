CREATE TABLE IF NOT EXISTS character_skills (
  character_id         INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  skill_id             INTEGER NOT NULL,
  active_skill_level   INTEGER NOT NULL,
  skillpoints_in_skill INTEGER NOT NULL,
  trained_skill_level  INTEGER NOT NULL,
  PRIMARY KEY (character_id, skill_id)
);
CREATE INDEX IF NOT EXISTS idx_character_skills_skill_id ON character_skills(skill_id);
