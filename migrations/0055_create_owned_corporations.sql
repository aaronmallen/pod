CREATE VIEW IF NOT EXISTS owned_corporations AS
SELECT
  c.id              AS id,
  c.alliance_id     AS alliance_id,
  c.ceo_id          AS ceo_id,
  c.home_station_id AS home_station_id,
  c.member_count    AS member_count,
  c.name            AS name,
  c.tax_rate        AS tax_rate,
  c.ticker          AS ticker,
  c.date_founded    AS date_founded,
  c.description     AS description,
  c.url             AS url,
  c.war_eligible    AS war_eligible,
  cr.authorized_by  AS authorized_by
FROM corporations c
JOIN credentials cr ON cr.owner_id = c.id AND cr.owner_type = 'corporation';
