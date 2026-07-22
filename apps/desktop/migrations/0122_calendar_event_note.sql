-- Captain's Log event notes: a short, user-authored note attached to a mirrored calendar event.
--
-- character_calendar (0065) rows are ESI-mirrored and overwritten on every sync, so a user's note
-- cannot live on that table. calendar_event_notes keys the note by (character_id, event_id) with
-- character_calendar's own semantics (character_id -> characters(id) ON DELETE CASCADE), keeping a
-- note independent of re-sync yet cleaned up when the character is removed.

CREATE TABLE IF NOT EXISTS calendar_event_notes (
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  event_id     INTEGER NOT NULL,
  note         TEXT    NOT NULL,
  created_at   TEXT    NOT NULL,
  updated_at   TEXT    NOT NULL,
  PRIMARY KEY (character_id, event_id)
);
CREATE INDEX IF NOT EXISTS idx_calendar_event_notes_character_id ON calendar_event_notes(character_id);
