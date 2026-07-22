CREATE VIEW IF NOT EXISTS mail_unified AS
SELECT
  m.character_id AS character_id,
  m.mail_id      AS mail_id,
  m.from_id      AS from_id,
  m.from_name    AS from_name,
  m.subject      AS subject,
  m.timestamp      AS timestamp,
  m.is_read        AS is_read,
  m.has_attachment AS has_attachment,
  m.important      AS important,
  m.from_corp      AS from_corp,
  m.from_system    AS from_system,
  b.body           AS body
FROM character_mail m
JOIN character_mail_body b ON b.character_id = m.character_id AND b.mail_id = m.mail_id;
