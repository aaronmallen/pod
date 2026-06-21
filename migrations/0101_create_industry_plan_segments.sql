-- One row per build-order job segment within a saved plan, keyed per built `type_id` to match the per-type
-- intent model (industry_plan_types): a job's runs can be split across several segments that always sum to the
-- type's total runs, each assignable to a pilot+clone. `segment_index` is the stable ordinal within a type.
-- `pilot_id` is the authenticated character id (NULL = unassigned); `clone_id` is the ESI jump_clone_id stored
-- opaquely and resolved at render (NULL = the pilot's active clone). Absence of any rows for a type is an
-- implicit single full segment (all runs, unassigned), so plans saved before this migration load unchanged.
CREATE TABLE IF NOT EXISTS industry_plan_segments (
  id              INTEGER PRIMARY KEY,
  plan_id         INTEGER NOT NULL REFERENCES industry_plans(id) ON DELETE CASCADE,
  type_id         INTEGER NOT NULL,
  segment_index   INTEGER NOT NULL,
  runs            INTEGER NOT NULL,
  pilot_id        INTEGER,
  clone_id        INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_industry_plan_segments ON industry_plan_segments(plan_id, type_id, segment_index);
