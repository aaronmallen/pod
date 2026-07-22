CREATE TABLE IF NOT EXISTS character_clones (
  character_id             INTEGER NOT NULL PRIMARY KEY REFERENCES characters(id) ON DELETE CASCADE,
  home_location_id         INTEGER NOT NULL,
  home_location_type       TEXT    NOT NULL CHECK(home_location_type IN ('station', 'structure')),
  home_location_name       TEXT,
  last_clone_jump_date     TEXT,
  last_station_change_date TEXT
);
