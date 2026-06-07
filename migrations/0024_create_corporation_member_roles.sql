CREATE TABLE IF NOT EXISTS corporation_member_roles (
  corporation_id INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  character_id   INTEGER NOT NULL,
  role           TEXT    NOT NULL,
  PRIMARY KEY (corporation_id, character_id, role)
);
CREATE INDEX IF NOT EXISTS idx_corporation_member_roles_corp_role
  ON corporation_member_roles(corporation_id, role);
CREATE INDEX IF NOT EXISTS idx_corporation_member_roles_character
  ON corporation_member_roles(character_id);
