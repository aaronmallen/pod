-- pod-telemetry D1 migration 0002 (spec mmmzstpq §6.2).
--
-- The environment size column was misnamed: `display` actually held the app's
-- own main-window logical size, so it becomes `window_size` to match the renamed
-- worker contract. RENAME COLUMN preserves existing rows' historical values.
-- A nullable `screen_size` column is added for the primary monitor's logical
-- size; rows from clients that predate the field stay NULL.
--
-- Forward-only; applies cleanly on a DB initialized from 0001_init.sql. D1
-- (SQLite 3.25+) supports both statements.

ALTER TABLE events RENAME COLUMN display TO window_size;
ALTER TABLE events ADD COLUMN screen_size TEXT;
