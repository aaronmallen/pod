CREATE TABLE IF NOT EXISTS skill_plans (
  id           INTEGER PRIMARY KEY,
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  name         TEXT    NOT NULL DEFAULT 'Untitled plan',
  sort_mode    TEXT    NOT NULL DEFAULT 'manual',
  implant_set  TEXT    NOT NULL DEFAULT 'current',
  created_at   TEXT    NOT NULL,
  updated_at   TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_skill_plans_character ON skill_plans(character_id);

CREATE TABLE IF NOT EXISTS skill_plan_entries (
  id       INTEGER PRIMARY KEY,
  plan_id  INTEGER NOT NULL REFERENCES skill_plans(id) ON DELETE CASCADE,
  skill_id INTEGER NOT NULL,
  to_level INTEGER NOT NULL CHECK (to_level BETWEEN 1 AND 5),
  position INTEGER NOT NULL,
  priority TEXT    NOT NULL DEFAULT 'normal',
  note     TEXT    NOT NULL DEFAULT '',
  is_auto  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_skill_plan_entries_plan ON skill_plan_entries(plan_id, position);
CREATE UNIQUE INDEX IF NOT EXISTS uq_skill_plan_entries_slot ON skill_plan_entries(plan_id, skill_id, to_level);

CREATE TABLE IF NOT EXISTS skill_plan_remap_points (
  id                INTEGER PRIMARY KEY,
  plan_id           INTEGER NOT NULL REFERENCES skill_plans(id) ON DELETE CASCADE,
  after_entry_id    INTEGER REFERENCES skill_plan_entries(id) ON DELETE CASCADE,
  base_perception   INTEGER NOT NULL CHECK (base_perception BETWEEN 17 AND 27),
  base_memory       INTEGER NOT NULL CHECK (base_memory BETWEEN 17 AND 27),
  base_willpower    INTEGER NOT NULL CHECK (base_willpower BETWEEN 17 AND 27),
  base_intelligence INTEGER NOT NULL CHECK (base_intelligence BETWEEN 17 AND 27),
  base_charisma     INTEGER NOT NULL CHECK (base_charisma BETWEEN 17 AND 27),
  -- EVE remap invariant: each base attribute is 17..27 and the five always sum to 99.
  CHECK (base_perception + base_memory + base_willpower + base_intelligence + base_charisma = 99)
);
CREATE INDEX IF NOT EXISTS idx_skill_plan_remap_plan ON skill_plan_remap_points(plan_id);

CREATE TABLE IF NOT EXISTS skill_plan_ship_masteries (
  plan_id      INTEGER NOT NULL REFERENCES skill_plans(id) ON DELETE CASCADE,
  ship_type_id INTEGER NOT NULL,
  tier         INTEGER NOT NULL,
  PRIMARY KEY (plan_id, ship_type_id)
);

CREATE TABLE IF NOT EXISTS skill_plan_cert_proficiencies (
  plan_id INTEGER NOT NULL REFERENCES skill_plans(id) ON DELETE CASCADE,
  cert_id INTEGER NOT NULL,
  level   INTEGER NOT NULL,
  PRIMARY KEY (plan_id, cert_id)
);
