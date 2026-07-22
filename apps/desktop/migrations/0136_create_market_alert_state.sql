CREATE TABLE IF NOT EXISTS market_alert_state (
  kind         TEXT    NOT NULL CHECK (kind IN ('outbid', 'target')),
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  subject_id   INTEGER NOT NULL,
  alerted      INTEGER NOT NULL DEFAULT 0,
  marker       TEXT    NOT NULL DEFAULT '',
  created_at   TEXT    NOT NULL,
  updated_at   TEXT    NOT NULL,
  PRIMARY KEY (kind, character_id, subject_id)
);

CREATE INDEX IF NOT EXISTS idx_market_alert_state_character ON market_alert_state(character_id);
