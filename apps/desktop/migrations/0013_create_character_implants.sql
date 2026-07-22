CREATE TABLE IF NOT EXISTS character_implants (
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  -- 164..168 are the EVE dogma attribute ids for charisma, intelligence, memory, perception, willpower.
  attribute_id INTEGER NOT NULL CHECK(attribute_id BETWEEN 164 AND 168),
  bonus        INTEGER NOT NULL,
  PRIMARY KEY (character_id, attribute_id)
);
