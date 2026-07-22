-- Skill completion history: a durable record of detected skill completions so "skills completed on
-- day X" is reconstructable after the fact.
--
-- The calendar's skill entries are synthetic overlays computed from the live skillqueue and vanish
-- the moment a skill finishes, leaving no row anywhere. skill_completion is written the instant the
-- shell notifications detector fires (capture) and reconciled against the next skillqueue sync
-- (verify or delete): a confirming sync flips verified, a contradicting one deletes the row. Forward
-- looking only — there is no backfill of pre-feature completions. The unique key keeps a re-detected
-- completion idempotent per (character, skill, level). sqlx runs the migration in one transaction.

CREATE TABLE IF NOT EXISTS skill_completion (
  id           INTEGER NOT NULL PRIMARY KEY,
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  skill_id     INTEGER NOT NULL,
  level        INTEGER NOT NULL,
  completed_at TEXT    NOT NULL,
  verified     INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT    NOT NULL,
  updated_at   TEXT    NOT NULL,
  UNIQUE (character_id, skill_id, level)
);
CREATE INDEX IF NOT EXISTS idx_skill_completion_character_id ON skill_completion(character_id);
