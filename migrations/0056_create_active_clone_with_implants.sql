CREATE VIEW IF NOT EXISTS active_clone_with_implants AS
SELECT
  c.character_id             AS character_id,
  c.home_location_id         AS home_location_id,
  c.home_location_type       AS home_location_type,
  c.home_location_name       AS home_location_name,
  c.last_clone_jump_date     AS last_clone_jump_date,
  c.last_station_change_date AS last_station_change_date,
  i.type_id                  AS implant_type_id,
  i.name                     AS implant_name,
  i.icon                     AS implant_icon
FROM character_clones c
LEFT JOIN character_clone_implants i
  ON i.character_id = c.character_id AND i.clone_id IS NULL;
