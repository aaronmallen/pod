CREATE TABLE IF NOT EXISTS sync_ledger (
  subject_type     TEXT    NOT NULL CHECK (subject_type IN ('character', 'corporation')),
  subject_id       INTEGER NOT NULL,
  kind             TEXT    NOT NULL,
  outcome          TEXT    NOT NULL CHECK (outcome IN ('synced', 'empty', 'blocked', 'not_ready', 'failed', 'skipped')),
  rows_touched     INTEGER NOT NULL DEFAULT 0,
  last_reason      TEXT,
  last_attempt_at  TEXT    NOT NULL,
  last_success_at  TEXT,
  next_eligible_at TEXT,
  PRIMARY KEY (subject_type, subject_id, kind)
);
