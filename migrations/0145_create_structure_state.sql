CREATE TABLE IF NOT EXISTS structure_state (
  structure_id           INTEGER PRIMARY KEY NOT NULL REFERENCES structures(id) ON DELETE CASCADE,
  fuel_expires           TEXT,
  state                  TEXT,
  services               TEXT    NOT NULL DEFAULT '[]',
  reinforce_hour         INTEGER,
  next_reinforce_apply   TEXT,
  next_reinforce_hour    INTEGER,
  next_reinforce_weekday INTEGER,
  state_timer_start      TEXT,
  state_timer_end        TEXT,
  unanchors_at           TEXT,
  synced_at              TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_structure_state_synced_at ON structure_state(synced_at);
