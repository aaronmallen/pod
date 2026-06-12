CREATE TABLE IF NOT EXISTS character_calendar_attendees (
  character_id   INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  event_id       INTEGER NOT NULL,
  attendee_id    INTEGER NOT NULL,
  event_response TEXT    NOT NULL,
  PRIMARY KEY (character_id, event_id, attendee_id)
);
CREATE INDEX IF NOT EXISTS idx_character_calendar_attendees_character_id          ON character_calendar_attendees(character_id);
CREATE INDEX IF NOT EXISTS idx_character_calendar_attendees_character_id_event_id ON character_calendar_attendees(character_id, event_id);
