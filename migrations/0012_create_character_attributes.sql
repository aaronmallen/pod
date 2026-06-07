CREATE TABLE IF NOT EXISTS character_attributes (
  character_id                 INTEGER PRIMARY KEY REFERENCES characters(id) ON DELETE CASCADE,
  charisma                     INTEGER NOT NULL,
  intelligence                 INTEGER NOT NULL,
  memory                       INTEGER NOT NULL,
  perception                   INTEGER NOT NULL,
  willpower                    INTEGER NOT NULL,
  bonus_remaps                 INTEGER NOT NULL,
  unallocated_sp               INTEGER NOT NULL DEFAULT 0,
  last_remap_date              TEXT,
  accrued_remap_cooldown_date  TEXT
);
