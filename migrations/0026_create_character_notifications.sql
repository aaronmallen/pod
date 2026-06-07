CREATE TABLE IF NOT EXISTS character_notifications (
  character_id    INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  notification_id INTEGER NOT NULL,
  notif_type      TEXT    NOT NULL,
  sender_id       INTEGER,
  sender_type     TEXT,
  timestamp       TEXT    NOT NULL,
  is_read         INTEGER NOT NULL DEFAULT 0,
  text            TEXT,
  synced_at       TEXT    NOT NULL,
  PRIMARY KEY (character_id, notification_id)
);
CREATE INDEX IF NOT EXISTS idx_character_notifications_character_id ON character_notifications(character_id);
CREATE INDEX IF NOT EXISTS idx_character_notifications_character_id_timestamp
  ON character_notifications(character_id, timestamp);
