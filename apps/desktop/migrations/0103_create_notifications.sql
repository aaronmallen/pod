-- In-app notification ledger (epic zyrmyrlk, spec A). One row per notified event. dedup_key is the
-- notify-exactly-once-ever watermark: its UNIQUE constraint makes the generic emit path
-- insert-if-absent, so re-running a detector over already-seen data is a no-op. First-run backfill
-- inserts rows with suppressed=1 so they occupy the dedup_key but never surface in the center/toaster,
-- which is how adding a character avoids flooding the feed. target_* carry the typed deep-link so the
-- UI can route on click. Named "notifications" (not the EVE in-game CharacterNotifications feed).

CREATE TABLE IF NOT EXISTS notifications (
  id          INTEGER PRIMARY KEY NOT NULL,
  kind        TEXT    NOT NULL,
  owner_type  TEXT    NOT NULL CHECK (owner_type IN ('character', 'corporation')),
  owner_id    INTEGER NOT NULL,
  dedup_key   TEXT    NOT NULL UNIQUE,
  title       TEXT    NOT NULL,
  body        TEXT    NOT NULL,
  target_dest TEXT    NOT NULL,
  target_char INTEGER,
  target_sub  TEXT,
  created_at  TEXT    NOT NULL,
  read_at     TEXT,
  suppressed  INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_notifications_dedup_key ON notifications(dedup_key);
CREATE INDEX IF NOT EXISTS idx_notifications_read_at ON notifications(read_at);
CREATE INDEX IF NOT EXISTS idx_notifications_created_at ON notifications(created_at);
