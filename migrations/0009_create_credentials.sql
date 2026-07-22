CREATE TABLE IF NOT EXISTS credentials (
  owner_id      INTEGER NOT NULL,
  owner_type    TEXT    NOT NULL CHECK(owner_type IN ('character', 'corporation')),
  access_token  TEXT    NOT NULL,
  refresh_token TEXT    NOT NULL,
  expires_at    INTEGER NOT NULL,
  authorized_by INTEGER,
  scopes        TEXT,
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL,
  PRIMARY KEY (owner_id, owner_type)
);
