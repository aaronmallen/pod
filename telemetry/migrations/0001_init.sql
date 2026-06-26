-- pod-telemetry D1 schema (spec mmmzstpq §6.2).
--
-- This is the Worker's OWN migration set, separate from the Rust app's
-- migrations/. Two fact tables; the Worker INSERTs inside one batched
-- transaction per envelope, then returns 204.
--
-- NO IP column exists anywhere: the Worker never reads or stores
-- CF-Connecting-IP / request.cf.

CREATE TABLE events (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  anon_id       TEXT    NOT NULL,   -- envelope.id (sha256(machine_id), 64 lc hex)
  session       TEXT    NOT NULL,
  schema        INTEGER NOT NULL,
  app_version   TEXT    NOT NULL,   -- envelope.app.version
  git_sha       TEXT,
  stream        TEXT    NOT NULL,   -- 'usage' | 'performance' | 'environment'
  event_kind    TEXT,              -- usage: view_open|feature_toggle|sub_section; else NULL
  name          TEXT,              -- route/feature token, or perf view name; NULL for env
  toggle_on     INTEGER,           -- 0/1 for feature_toggle, else NULL
  load_ms       INTEGER,
  frame_p95_ms  INTEGER,
  heap_mb       INTEGER,
  os            TEXT,              -- environment rows only
  os_version    TEXT,
  arch          TEXT,
  display       TEXT,
  locale        TEXT,
  event_at      TEXT    NOT NULL,  -- usage.t, else sent_at
  received_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);
CREATE INDEX idx_events_received ON events(received_at);
CREATE INDEX idx_events_anon     ON events(anon_id);
CREATE INDEX idx_events_stream   ON events(stream, name);

CREATE TABLE crashes (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  anon_id       TEXT    NOT NULL,  -- of the crashed run
  session       TEXT    NOT NULL,  -- of the crashed run
  schema        INTEGER NOT NULL,
  app_version   TEXT    NOT NULL,  -- version that crashed
  git_sha       TEXT,
  message       TEXT    NOT NULL,
  location      TEXT,
  backtrace     TEXT,              -- JSON array of frame strings (TEXT)
  context_log   TEXT,              -- JSON array of scrubbed log lines (TEXT)
  crashed_at    TEXT    NOT NULL,  -- when the panic happened (from buffer)
  received_at   TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);
CREATE INDEX idx_crashes_received ON crashes(received_at);
CREATE INDEX idx_crashes_group    ON crashes(app_version, message);
