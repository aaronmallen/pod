-- Skill Plan Templates: make skill_plans.character_id nullable and add an is_template flag
-- (default 0) so account-wide, character-independent template plans persist alongside the
-- existing per-character plans.
--
-- SQLite cannot drop NOT NULL in place, so skill_plans is rebuilt. With foreign_keys ON,
-- DROP TABLE performs an implicit DELETE whose ON DELETE CASCADE actions would wipe the four
-- child tables, so each child is snapshotted to a plain copy, dropped before the parent,
-- recreated verbatim from 0027 afterward, and restored. sqlx runs the migration inside one
-- transaction, so a crash mid-rebuild rolls back to the pre-migration tables.

CREATE TABLE skill_plans_new (
  id           INTEGER PRIMARY KEY,
  character_id INTEGER REFERENCES characters(id) ON DELETE CASCADE,
  name         TEXT    NOT NULL DEFAULT 'Untitled plan',
  sort_mode    TEXT    NOT NULL DEFAULT 'manual',
  implant_set  TEXT    NOT NULL DEFAULT 'current',
  is_template  INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT    NOT NULL,
  updated_at   TEXT    NOT NULL
);

INSERT INTO skill_plans_new (id, character_id, name, sort_mode, implant_set, is_template, created_at, updated_at)
SELECT id, character_id, name, sort_mode, implant_set, 0, created_at, updated_at
FROM skill_plans;

CREATE TABLE skill_plan_entries_copy AS
SELECT id, plan_id, skill_id, to_level, position, priority, note, is_auto
FROM skill_plan_entries;

CREATE TABLE skill_plan_remap_points_copy AS
SELECT id, plan_id, after_entry_id, base_perception, base_memory, base_willpower, base_intelligence, base_charisma
FROM skill_plan_remap_points;

CREATE TABLE skill_plan_ship_masteries_copy AS
SELECT plan_id, ship_type_id, tier
FROM skill_plan_ship_masteries;

CREATE TABLE skill_plan_cert_proficiencies_copy AS
SELECT plan_id, cert_id, level
FROM skill_plan_cert_proficiencies;

DROP TABLE skill_plan_remap_points;
DROP TABLE skill_plan_entries;
DROP TABLE skill_plan_ship_masteries;
DROP TABLE skill_plan_cert_proficiencies;
DROP TABLE skill_plans;

ALTER TABLE skill_plans_new RENAME TO skill_plans;

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

INSERT INTO skill_plan_entries (id, plan_id, skill_id, to_level, position, priority, note, is_auto)
SELECT id, plan_id, skill_id, to_level, position, priority, note, is_auto
FROM skill_plan_entries_copy;

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

INSERT INTO skill_plan_remap_points
  (id, plan_id, after_entry_id, base_perception, base_memory, base_willpower, base_intelligence, base_charisma)
SELECT id, plan_id, after_entry_id, base_perception, base_memory, base_willpower, base_intelligence, base_charisma
FROM skill_plan_remap_points_copy;

CREATE TABLE IF NOT EXISTS skill_plan_ship_masteries (
  plan_id      INTEGER NOT NULL REFERENCES skill_plans(id) ON DELETE CASCADE,
  ship_type_id INTEGER NOT NULL,
  tier         INTEGER NOT NULL,
  PRIMARY KEY (plan_id, ship_type_id)
);

INSERT INTO skill_plan_ship_masteries (plan_id, ship_type_id, tier)
SELECT plan_id, ship_type_id, tier
FROM skill_plan_ship_masteries_copy;

CREATE TABLE IF NOT EXISTS skill_plan_cert_proficiencies (
  plan_id INTEGER NOT NULL REFERENCES skill_plans(id) ON DELETE CASCADE,
  cert_id INTEGER NOT NULL,
  level   INTEGER NOT NULL,
  PRIMARY KEY (plan_id, cert_id)
);

INSERT INTO skill_plan_cert_proficiencies (plan_id, cert_id, level)
SELECT plan_id, cert_id, level
FROM skill_plan_cert_proficiencies_copy;

DROP TABLE skill_plan_entries_copy;
DROP TABLE skill_plan_remap_points_copy;
DROP TABLE skill_plan_ship_masteries_copy;
DROP TABLE skill_plan_cert_proficiencies_copy;
