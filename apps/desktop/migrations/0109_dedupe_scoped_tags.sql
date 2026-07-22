-- Enforce unique tag names per scope: (scope, lower(name)).
-- First a repair pass collapses any pre-existing duplicates, then the unique index is created. Both
-- steps are idempotent and safe on a database that already has zero duplicates.

-- Survivor per (scope, lower(name)) group: prefer a row with a non-empty color, else the lowest id.
-- The CASE ranks coloured rows ahead of uncoloured ones; id breaks remaining ties.
CREATE TEMP TABLE tag_dedupe_survivors AS
SELECT
  g.scope AS scope,
  lower(g.name) AS lname,
  (
    SELECT t.id FROM tags t
    WHERE t.scope = g.scope AND lower(t.name) = lower(g.name)
    ORDER BY CASE WHEN t.color IS NOT NULL AND t.color <> '' THEN 0 ELSE 1 END, t.id
    LIMIT 1
  ) AS survivor_id
FROM tags g
GROUP BY g.scope, lower(g.name)
HAVING COUNT(*) > 1;

-- Re-point memberships of losing tags onto the survivor without tripping the entity_tags primary key:
-- an entity already carrying the survivor keeps its single membership row.
INSERT OR IGNORE INTO entity_tags (tag_id, entity_type, entity_id)
SELECT s.survivor_id, et.entity_type, et.entity_id
FROM entity_tags et
JOIN tags loser ON loser.id = et.tag_id
JOIN tag_dedupe_survivors s ON s.scope = loser.scope AND s.lname = lower(loser.name)
WHERE et.tag_id <> s.survivor_id;

-- Drop the now-migrated memberships of the losing tags.
DELETE FROM entity_tags
WHERE tag_id IN (
  SELECT loser.id
  FROM tags loser
  JOIN tag_dedupe_survivors s ON s.scope = loser.scope AND s.lname = lower(loser.name)
  WHERE loser.id <> s.survivor_id
);

-- Delete the losing tag rows, leaving exactly one row per (scope, lower(name)).
DELETE FROM tags
WHERE id IN (
  SELECT loser.id
  FROM tags loser
  JOIN tag_dedupe_survivors s ON s.scope = loser.scope AND s.lname = lower(loser.name)
  WHERE loser.id <> s.survivor_id
);

DROP TABLE tag_dedupe_survivors;

CREATE UNIQUE INDEX IF NOT EXISTS uq_tags_scope_lower_name ON tags(scope, lower(name));
