CREATE TABLE IF NOT EXISTS outbox (
  id              INTEGER PRIMARY KEY,
  subject_type    TEXT    NOT NULL,
  subject_id      INTEGER NOT NULL,
  kind            TEXT    NOT NULL,
  payload         TEXT    NOT NULL,
  dedupe_key      TEXT,
  status          TEXT    NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'inflight', 'done', 'failed')),
  attempts        INTEGER NOT NULL DEFAULT 0,
  next_attempt_at TEXT    NOT NULL,
  last_error      TEXT,
  created_at      TEXT    NOT NULL,
  updated_at      TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_outbox_drainable ON outbox(status, next_attempt_at);
CREATE UNIQUE INDEX IF NOT EXISTS uq_outbox_dedupe
  ON outbox(subject_id, kind, dedupe_key) WHERE dedupe_key IS NOT NULL AND status IN ('pending', 'inflight');
