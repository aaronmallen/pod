-- One row per node in a saved plan's recursive build tree, including the root product node. A node's
-- identity is a materialized path: the '/'-joined chain of material type-ids walked from the root down
-- (root = empty string ''), mirroring the planner's path-of-type-ids tree keying. Storing the path
-- rather than a parent-pointer lets a saved tree be rehydrated in a single ordered scan with no
-- recursive self-join, and matches how the planner addresses nodes at runtime.
CREATE TABLE IF NOT EXISTS industry_plan_nodes (
  id              INTEGER PRIMARY KEY,
  plan_id         INTEGER NOT NULL REFERENCES industry_plans(id) ON DELETE CASCADE,
  path            TEXT    NOT NULL,
  me              INTEGER NOT NULL,
  te              INTEGER NOT NULL,
  facility_system INTEGER
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_industry_plan_nodes_path ON industry_plan_nodes(plan_id, path);
