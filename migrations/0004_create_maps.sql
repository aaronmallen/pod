CREATE TABLE IF NOT EXISTS regions (
  id          INTEGER PRIMARY KEY NOT NULL,
  name        TEXT    NOT NULL,
  description TEXT
);

CREATE TABLE IF NOT EXISTS constellations (
  id        INTEGER PRIMARY KEY NOT NULL,
  region_id INTEGER NOT NULL REFERENCES regions(id),
  name      TEXT    NOT NULL,
  position_x REAL    NOT NULL,
  position_y REAL    NOT NULL,
  position_z REAL    NOT NULL
);

CREATE TABLE IF NOT EXISTS solar_systems (
  id               INTEGER PRIMARY KEY NOT NULL,
  constellation_id INTEGER NOT NULL REFERENCES constellations(id),
  name             TEXT    NOT NULL,
  position_x       REAL    NOT NULL,
  position_y       REAL    NOT NULL,
  position_z       REAL    NOT NULL,
  security_status  REAL    NOT NULL,
  security_class   TEXT,
  star_id          INTEGER
);

CREATE TABLE IF NOT EXISTS stations (
  id                        INTEGER PRIMARY KEY NOT NULL,
  system_id                 INTEGER NOT NULL REFERENCES solar_systems(id),
  type_id                   INTEGER NOT NULL REFERENCES item_types(id),
  name                      TEXT    NOT NULL,
  max_dockable_ship_volume  REAL    NOT NULL,
  office_rental_cost        REAL    NOT NULL,
  reprocessing_efficiency   REAL    NOT NULL,
  reprocessing_stations_take REAL   NOT NULL,
  services                  TEXT    NOT NULL DEFAULT '[]',
  owner                     INTEGER REFERENCES corporations(id),
  position_x                REAL    NOT NULL,
  position_y                REAL    NOT NULL,
  position_z                REAL    NOT NULL,
  race_id                   INTEGER REFERENCES races(id)
);

CREATE TABLE IF NOT EXISTS structures (
  id              INTEGER PRIMARY KEY NOT NULL,
  solar_system_id INTEGER NOT NULL REFERENCES solar_systems(id),
  owner_id        INTEGER NOT NULL REFERENCES corporations(id),
  type_id         INTEGER REFERENCES item_types(id),
  name            TEXT    NOT NULL,
  position_x      REAL,
  position_y      REAL,
  position_z      REAL
);

CREATE TABLE IF NOT EXISTS inaccessible_structures (
  owner_id   INTEGER NOT NULL,
  owner_type TEXT    NOT NULL,
  id         INTEGER NOT NULL,
  marked_at  TEXT    NOT NULL,
  PRIMARY KEY (owner_id, owner_type, id)
);

CREATE INDEX IF NOT EXISTS idx_constellations_region_id     ON constellations(region_id);
CREATE INDEX IF NOT EXISTS idx_solar_systems_constellation  ON solar_systems(constellation_id);
CREATE INDEX IF NOT EXISTS idx_solar_systems_star_id        ON solar_systems(star_id);
CREATE INDEX IF NOT EXISTS idx_stations_owner               ON stations(owner);
CREATE INDEX IF NOT EXISTS idx_stations_race_id             ON stations(race_id);
CREATE INDEX IF NOT EXISTS idx_stations_system_id           ON stations(system_id);
CREATE INDEX IF NOT EXISTS idx_stations_type_id             ON stations(type_id);
CREATE INDEX IF NOT EXISTS idx_structures_owner_id          ON structures(owner_id);
CREATE INDEX IF NOT EXISTS idx_structures_solar_system_id   ON structures(solar_system_id);
CREATE INDEX IF NOT EXISTS idx_structures_type_id           ON structures(type_id);
