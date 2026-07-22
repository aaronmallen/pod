-- Activity-level build-time metadata seeded from the SDE blueprints.yaml. `time` is the base seconds per
-- run/cycle and `max_production_limit` is the per-job run cap. Both are properties of the activity, not of
-- any single product or material, so they live in a dedicated table keyed by (blueprint_type_id,
-- activity_id) rather than duplicated across every blueprint_activity_products/materials row.
CREATE TABLE IF NOT EXISTS blueprint_activity_meta (
  blueprint_type_id    INTEGER NOT NULL,
  activity_id          INTEGER NOT NULL,
  time                 INTEGER NOT NULL,
  max_production_limit INTEGER NOT NULL,
  PRIMARY KEY (blueprint_type_id, activity_id)
);
