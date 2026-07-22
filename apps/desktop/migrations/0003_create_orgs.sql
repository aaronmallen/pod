
CREATE TABLE IF NOT EXISTS alliances (
  id                      INTEGER PRIMARY KEY NOT NULL,
  creator_corporation_id  INTEGER NOT NULL,
  creator_id              INTEGER NOT NULL,
  date_founded            TEXT    NOT NULL,
  executor_corporation_id INTEGER,
  faction_id              INTEGER,
  name                    TEXT    NOT NULL,
  ticker                  TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS corporations (
  id              INTEGER PRIMARY KEY NOT NULL,
  -- DEFERRABLE: corporations and characters reference each other's parents out of order during seed/sync; defer the FK to the end of the transaction.
  alliance_id     INTEGER REFERENCES alliances(id) DEFERRABLE INITIALLY DEFERRED,
  ceo_id          INTEGER NOT NULL,
  creator_id      INTEGER NOT NULL,
  faction_id      INTEGER,
  home_station_id INTEGER,
  member_count    INTEGER NOT NULL,
  name            TEXT    NOT NULL,
  tax_rate        REAL    NOT NULL,
  ticker          TEXT    NOT NULL,
  date_founded    TEXT,
  description     TEXT,
  shares          INTEGER,
  url             TEXT,
  war_eligible    INTEGER
);

CREATE TABLE IF NOT EXISTS races (
  id          INTEGER PRIMARY KEY NOT NULL,
  alliance_id INTEGER NOT NULL,
  description TEXT    NOT NULL,
  name        TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS bloodlines (
  id             INTEGER PRIMARY KEY NOT NULL,
  corporation_id INTEGER NOT NULL,
  race_id        INTEGER NOT NULL REFERENCES races(id),
  ship_type_id   INTEGER,
  charisma       INTEGER NOT NULL,
  description    TEXT    NOT NULL,
  intelligence   INTEGER NOT NULL,
  memory         INTEGER NOT NULL,
  name           TEXT    NOT NULL,
  perception     INTEGER NOT NULL,
  willpower      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS factions (
  id                     INTEGER PRIMARY KEY NOT NULL,
  corporation_id         INTEGER,
  militia_corporation_id INTEGER,
  solar_system_id        INTEGER,
  description            TEXT    NOT NULL,
  is_unique              INTEGER NOT NULL,
  name                   TEXT    NOT NULL,
  size_factor            REAL    NOT NULL,
  station_count          INTEGER NOT NULL,
  station_system_count   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS characters (
  id              INTEGER PRIMARY KEY NOT NULL,
  bloodline_id    INTEGER NOT NULL REFERENCES bloodlines(id),
  -- DEFERRABLE: a character can be inserted before its corporation row exists during sync; defer the FK to the end of the transaction.
  corporation_id  INTEGER NOT NULL REFERENCES corporations(id) DEFERRABLE INITIALLY DEFERRED,
  race_id         INTEGER NOT NULL REFERENCES races(id),
  alliance_id     INTEGER REFERENCES alliances(id),
  faction_id      INTEGER REFERENCES factions(id),
  birthday        TEXT    NOT NULL,
  gender          TEXT    NOT NULL CHECK(gender IN ('female', 'male')),
  name            TEXT    NOT NULL,
  description     TEXT,
  security_status REAL,
  title           TEXT
);

CREATE INDEX IF NOT EXISTS idx_alliances_creator_corporation ON alliances(creator_corporation_id);
CREATE INDEX IF NOT EXISTS idx_alliances_creator_id          ON alliances(creator_id);
CREATE INDEX IF NOT EXISTS idx_alliances_executor_corp       ON alliances(executor_corporation_id);
CREATE INDEX IF NOT EXISTS idx_alliances_faction_id          ON alliances(faction_id);
CREATE INDEX IF NOT EXISTS idx_bloodlines_corporation_id     ON bloodlines(corporation_id);
CREATE INDEX IF NOT EXISTS idx_bloodlines_race_id            ON bloodlines(race_id);
CREATE INDEX IF NOT EXISTS idx_bloodlines_ship_type_id       ON bloodlines(ship_type_id);
CREATE INDEX IF NOT EXISTS idx_characters_alliance_id        ON characters(alliance_id);
CREATE INDEX IF NOT EXISTS idx_characters_bloodline_id       ON characters(bloodline_id);
CREATE INDEX IF NOT EXISTS idx_characters_corporation_id     ON characters(corporation_id);
CREATE INDEX IF NOT EXISTS idx_characters_faction_id         ON characters(faction_id);
CREATE INDEX IF NOT EXISTS idx_characters_race_id            ON characters(race_id);
CREATE INDEX IF NOT EXISTS idx_corporations_alliance_id      ON corporations(alliance_id);
CREATE INDEX IF NOT EXISTS idx_corporations_ceo_id           ON corporations(ceo_id);
CREATE INDEX IF NOT EXISTS idx_corporations_creator_id       ON corporations(creator_id);
CREATE INDEX IF NOT EXISTS idx_corporations_faction_id       ON corporations(faction_id);
CREATE INDEX IF NOT EXISTS idx_corporations_home_station_id  ON corporations(home_station_id);
CREATE INDEX IF NOT EXISTS idx_factions_corporation_id       ON factions(corporation_id);
CREATE INDEX IF NOT EXISTS idx_factions_militia_corp_id      ON factions(militia_corporation_id);
CREATE INDEX IF NOT EXISTS idx_factions_solar_system_id      ON factions(solar_system_id);
CREATE INDEX IF NOT EXISTS idx_races_alliance_id             ON races(alliance_id);
