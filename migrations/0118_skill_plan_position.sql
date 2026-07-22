-- Skill Plan ordering: add a manual position column so plans and templates can be drag-reordered
-- in the manager instead of listing in fixed creation order.
--
-- Templates form one global sequence (all templates share is_template = 1 and a NULL character),
-- while each character's plans are ordered independently. A single window-function backfill seeds
-- position from the existing creation order (created_at, id) per (is_template, character_id) group,
-- so upgrading databases keep their current order as the initial arrangement. position is 0-based to
-- match skill_plan_entries and the reorder_plans enumerate write. sqlx runs the migration in one
-- transaction, so a crash mid-backfill rolls back to the pre-migration table.

ALTER TABLE skill_plans ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

WITH ordered AS (
  SELECT
    id,
    ROW_NUMBER() OVER (
      PARTITION BY is_template, character_id
      ORDER BY created_at, id
    ) - 1 AS pos
  FROM skill_plans
)
UPDATE skill_plans
SET position = (SELECT pos FROM ordered WHERE ordered.id = skill_plans.id);
