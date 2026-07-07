-- Captain's Log field notes: free-form, timestamped notes that don't fit the structured questionnaire.
--
-- Field notes are a running list per day, account-scoped: they belong to the log's day, not to any one
-- character, so the table keys on `date` alone with no character_id. Each row carries its own autoincrement
-- id (delete/edit target one note) plus created_at (immutable, drives the HH:MM stamp) and updated_at (touched
-- on edit). There is deliberately NO foreign key: captains_log rows are lazily created and a day may have field
-- notes with no captains_log entry at all, so a note must stand on its own. idx_field_notes_date serves the
-- per-day list. sqlx runs the migration in one transaction.

CREATE TABLE IF NOT EXISTS field_notes (
  id         INTEGER NOT NULL PRIMARY KEY,
  date       TEXT    NOT NULL,
  text       TEXT    NOT NULL,
  created_at TEXT    NOT NULL,
  updated_at TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_field_notes_date ON field_notes(date);
