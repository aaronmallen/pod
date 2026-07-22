-- Captain's Log storage: one account-scoped row per calendar day the user has authored content for.
--
-- A row holds the free-form narrative line plus the eight fixed prompt answers (goal, remember,
-- blocked, build, skill, combat, next, research), every field nullable so a partial day is valid.
-- The date (YYYY-MM-DD) is the primary key; there is no per-character scope. Rows exist only where
-- the user (or their MCP agent) explicitly wrote something, so reads never create rows and browsing
-- an empty day yields nothing.

CREATE TABLE IF NOT EXISTS captains_log (
  date       TEXT NOT NULL PRIMARY KEY,
  narrative  TEXT,
  goal       TEXT,
  remember   TEXT,
  blocked    TEXT,
  build      TEXT,
  skill      TEXT,
  combat     TEXT,
  next       TEXT,
  research   TEXT,
  marked_complete INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
