-- pod-telemetry D1 migration 0003 (spec mmmzstpq §6.2).
--
-- Adds a nullable `app_language` column for the user's chosen UI language (the
-- esi code, e.g. "en-us", "de"), distinct from the OS-derived `locale`. Rows
-- from clients that predate the field stay NULL and collapse to the "unknown"
-- bucket in the dashboard aggregation.
--
-- Forward-only; applies cleanly on a DB initialized from 0001_init.sql and
-- migrated through 0002. Old clients that omit the field keep recording.

ALTER TABLE events ADD COLUMN app_language TEXT;
