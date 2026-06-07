CREATE TABLE IF NOT EXISTS certificates (
  id          INTEGER PRIMARY KEY NOT NULL,
  name        TEXT    NOT NULL,
  description TEXT,
  grade       INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS certificate_skills (
  certificate_id INTEGER NOT NULL REFERENCES certificates(id),
  skill_id       INTEGER NOT NULL REFERENCES item_types(id),
  basic          INTEGER NOT NULL DEFAULT 0,
  improved       INTEGER NOT NULL DEFAULT 0,
  advanced       INTEGER NOT NULL DEFAULT 0,
  elite          INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (certificate_id, skill_id)
);

CREATE INDEX IF NOT EXISTS idx_certificate_skills_skill_id ON certificate_skills(skill_id);

CREATE TABLE IF NOT EXISTS ship_masteries (
  ship_type_id   INTEGER NOT NULL REFERENCES item_types(id),
  tier           INTEGER NOT NULL,
  certificate_id INTEGER NOT NULL REFERENCES certificates(id),
  PRIMARY KEY (ship_type_id, tier, certificate_id)
);

CREATE INDEX IF NOT EXISTS idx_ship_masteries_certificate_id ON ship_masteries(certificate_id);
