CREATE TABLE IF NOT EXISTS corporation_industry_jobs (
  job_id                INTEGER NOT NULL PRIMARY KEY,
  corporation_id        INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  activity_id           INTEGER NOT NULL,
  blueprint_id          INTEGER NOT NULL,
  blueprint_location_id INTEGER NOT NULL,
  blueprint_type_id     INTEGER NOT NULL,
  completed_character_id INTEGER,
  completed_date        TEXT,
  cost                  REAL,
  duration              INTEGER NOT NULL,
  end_date              TEXT    NOT NULL,
  facility_id           INTEGER NOT NULL,
  installer_id          INTEGER NOT NULL,
  licensed_runs         INTEGER,
  output_location_id    INTEGER NOT NULL,
  pause_date            TEXT,
  probability           REAL,
  product_type_id       INTEGER,
  runs                  INTEGER NOT NULL,
  start_date            TEXT    NOT NULL,
  station_id            INTEGER,
  status                TEXT    NOT NULL,
  successful_runs       INTEGER
);
CREATE INDEX IF NOT EXISTS idx_corporation_industry_jobs_corporation_id      ON corporation_industry_jobs(corporation_id);
CREATE INDEX IF NOT EXISTS idx_corporation_industry_jobs_owner_end_date      ON corporation_industry_jobs(corporation_id, end_date);
