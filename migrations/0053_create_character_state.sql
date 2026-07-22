CREATE VIEW IF NOT EXISTS character_state AS
SELECT
  c.id              AS character_id,
  t.online          AS online,
  t.solar_system_id AS solar_system_id,
  t.station_id      AS station_id,
  t.structure_id    AS structure_id,
  t.ship_item_id    AS ship_item_id,
  t.ship_name       AS ship_name,
  t.ship_type_id    AS ship_type_id,
  t.synced_at       AS synced_at,
  (SELECT SUM(s.skillpoints_in_skill)
    FROM character_skills s
    WHERE s.character_id = c.id)               AS total_sp,
  (
    SELECT anchor.balance + COALESCE((
      SELECT SUM(COALESCE(later.amount, 0))
        FROM character_wallet_journal later
        WHERE later.character_id = anchor.character_id AND later.id > anchor.id
    ), 0)
      FROM character_wallet_journal anchor
      WHERE anchor.character_id = c.id AND anchor.balance IS NOT NULL
      ORDER BY anchor.id DESC
      LIMIT 1
  )                                            AS wallet_balance
FROM characters c
LEFT JOIN character_telemetry t ON t.character_id = c.id;
