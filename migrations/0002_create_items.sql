CREATE TABLE IF NOT EXISTS item_categories (
  id        INTEGER PRIMARY KEY NOT NULL,
  name      TEXT    NOT NULL,
  published INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS item_groups (
  id          INTEGER PRIMARY KEY NOT NULL,
  category_id INTEGER NOT NULL REFERENCES item_categories(id),
  name        TEXT    NOT NULL,
  published   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS market_groups (
  id              INTEGER PRIMARY KEY NOT NULL,
  description     TEXT    NOT NULL,
  name            TEXT    NOT NULL,
  parent_group_id INTEGER REFERENCES market_groups(id)
);

CREATE TABLE IF NOT EXISTS item_types (
  id               INTEGER PRIMARY KEY NOT NULL,
  group_id         INTEGER NOT NULL REFERENCES item_groups(id),
  market_group_id  INTEGER REFERENCES market_groups(id),
  description      TEXT    NOT NULL,
  name             TEXT    NOT NULL,
  published        INTEGER NOT NULL,
  capacity         REAL,
  dogma_attributes TEXT    NOT NULL DEFAULT '[]',
  dogma_effects    TEXT    NOT NULL DEFAULT '[]',
  graphic_id       INTEGER,
  icon_id          INTEGER,
  mass             REAL,
  packaged_volume  REAL,
  portion_size     INTEGER,
  radius           REAL,
  volume           REAL
);

CREATE INDEX IF NOT EXISTS idx_item_groups_category_id    ON item_groups(category_id);
CREATE INDEX IF NOT EXISTS idx_item_types_graphic_id      ON item_types(graphic_id);
CREATE INDEX IF NOT EXISTS idx_item_types_group_id        ON item_types(group_id);
CREATE INDEX IF NOT EXISTS idx_item_types_icon_id         ON item_types(icon_id);
CREATE INDEX IF NOT EXISTS idx_item_types_market_group_id ON item_types(market_group_id);
CREATE INDEX IF NOT EXISTS idx_market_groups_parent_group ON market_groups(parent_group_id);
