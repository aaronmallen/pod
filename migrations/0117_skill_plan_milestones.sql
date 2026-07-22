-- Skill Plan Milestones: replace free-floating remap points with named section markers.
--
-- skill_plan_remap_points (0027) forces all five base_* NOT NULL with a sum=99 CHECK, so it
-- cannot express a named marker or a nullable base. skill_plan_milestones adds name, an
-- auto_remap flag, a position tiebreaker, and an all-or-nothing nullable base. Existing remap
-- rows fold into milestones named "Milestone N" (per-plan ordinal in anchor order: the start
-- bucket first, then by the anchored entry's position), then the old table is dropped.
--
-- skill_plan_remap_points is a leaf table (nothing references it), so it drops directly without
-- the parent-rebuild dance of 0114. sqlx runs the migration in one transaction.

CREATE TABLE skill_plan_milestones (
  id                INTEGER PRIMARY KEY,
  plan_id           INTEGER NOT NULL REFERENCES skill_plans(id) ON DELETE CASCADE,
  after_entry_id    INTEGER REFERENCES skill_plan_entries(id) ON DELETE CASCADE,
  name              TEXT    NOT NULL DEFAULT 'Milestone',
  auto_remap        INTEGER NOT NULL DEFAULT 0,
  position          INTEGER NOT NULL DEFAULT 0,
  base_perception   INTEGER CHECK (base_perception   BETWEEN 17 AND 27),
  base_memory       INTEGER CHECK (base_memory       BETWEEN 17 AND 27),
  base_willpower    INTEGER CHECK (base_willpower    BETWEEN 17 AND 27),
  base_intelligence INTEGER CHECK (base_intelligence BETWEEN 17 AND 27),
  base_charisma     INTEGER CHECK (base_charisma     BETWEEN 17 AND 27),
  -- All-or-nothing base: either every attribute is absent (a pure section boundary) or all five
  -- are present and honour the EVE remap invariant (each 17..27, the five summing to 99).
  CHECK (
    (base_perception IS NULL AND base_memory IS NULL AND base_willpower IS NULL
      AND base_intelligence IS NULL AND base_charisma IS NULL)
    OR
    (base_perception IS NOT NULL AND base_memory IS NOT NULL AND base_willpower IS NOT NULL
      AND base_intelligence IS NOT NULL AND base_charisma IS NOT NULL
      AND base_perception + base_memory + base_willpower + base_intelligence + base_charisma = 99)
  )
);
CREATE INDEX IF NOT EXISTS idx_skill_plan_milestones_plan ON skill_plan_milestones(plan_id);

INSERT INTO skill_plan_milestones
  (id, plan_id, after_entry_id, name, auto_remap, position,
   base_perception, base_memory, base_willpower, base_intelligence, base_charisma)
SELECT
  r.id,
  r.plan_id,
  r.after_entry_id,
  'Milestone ' || ROW_NUMBER() OVER (
    PARTITION BY r.plan_id
    ORDER BY (r.after_entry_id IS NOT NULL), COALESCE(e.position, -1), r.id
  ),
  0,
  ROW_NUMBER() OVER (
    PARTITION BY r.plan_id
    ORDER BY (r.after_entry_id IS NOT NULL), COALESCE(e.position, -1), r.id
  ),
  r.base_perception,
  r.base_memory,
  r.base_willpower,
  r.base_intelligence,
  r.base_charisma
FROM skill_plan_remap_points r
LEFT JOIN skill_plan_entries e ON e.id = r.after_entry_id;

DROP TABLE skill_plan_remap_points;
