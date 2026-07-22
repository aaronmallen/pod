CREATE TABLE IF NOT EXISTS dogma_attributes (
  attribute_id  INTEGER PRIMARY KEY NOT NULL,
  name          TEXT    NOT NULL,
  display_name  TEXT,
  description   TEXT,
  unit_id       INTEGER,
  icon_id       INTEGER,
  default_value REAL,
  high_is_good  INTEGER NOT NULL,
  published     INTEGER NOT NULL,
  stackable     INTEGER NOT NULL
);
