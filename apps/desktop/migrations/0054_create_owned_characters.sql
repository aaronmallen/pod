CREATE VIEW IF NOT EXISTS owned_characters AS
SELECT
  c.alliance_id      AS alliance_id,
  c.birthday         AS birthday,
  c.bloodline_id     AS bloodline_id,
  c.corporation_id   AS corporation_id,
  c.description       AS description,
  c.faction_id       AS faction_id,
  c.gender           AS gender,
  c.id               AS id,
  c.name             AS name,
  c.race_id          AS race_id,
  c.security_status  AS security_status,
  c.title            AS title
FROM characters c
JOIN credentials cr ON cr.owner_id = c.id AND cr.owner_type = 'character';
