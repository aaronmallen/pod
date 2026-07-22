CREATE TABLE IF NOT EXISTS saved_asset_filters (
  id       INTEGER NOT NULL PRIMARY KEY,
  name     TEXT    NOT NULL,
  query    TEXT    NOT NULL DEFAULT '',
  category TEXT
);
