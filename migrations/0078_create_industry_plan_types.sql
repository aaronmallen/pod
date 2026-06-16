-- One row per distinct built item type in a saved plan, keyed by `type_id` rather than by tree path. A
-- plan's user intent is keyed by item TYPE, not by the position a job occupies in the computed build tree:
-- editing ME/TE/facility for a type applies to every occurrence, and the recursive build tree is DERIVED
-- offline by walking recipes from the product and descending only into types flagged `built`. The root
-- product is itself a row (its own ME/TE/facility); `built` defaults to 0 and is 1 for every type the user
-- chose to produce in-house.
CREATE TABLE IF NOT EXISTS industry_plan_types (
  id              INTEGER PRIMARY KEY,
  plan_id         INTEGER NOT NULL REFERENCES industry_plans(id) ON DELETE CASCADE,
  type_id         INTEGER NOT NULL,
  me              INTEGER NOT NULL,
  te              INTEGER NOT NULL,
  facility_system INTEGER,
  built           INTEGER NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_industry_plan_types ON industry_plan_types(plan_id, type_id);
