ALTER TABLE character_killmails ADD COLUMN victim_alliance_id INTEGER;
ALTER TABLE character_killmails ADD COLUMN victim_damage_taken INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS killmail_attackers (
  character_id          INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  killmail_id           INTEGER NOT NULL,
  ordinal               INTEGER NOT NULL,
  attacker_character_id INTEGER,
  corporation_id        INTEGER,
  alliance_id           INTEGER,
  ship_type_id          INTEGER,
  damage_done           INTEGER NOT NULL DEFAULT 0,
  final_blow            INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (character_id, killmail_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_killmail_attackers_character_id_killmail_id ON killmail_attackers(character_id, killmail_id);

CREATE TABLE IF NOT EXISTS killmail_items (
  character_id       INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  killmail_id        INTEGER NOT NULL,
  ordinal            INTEGER NOT NULL,
  type_id            INTEGER NOT NULL,
  flag               INTEGER NOT NULL,
  quantity_destroyed INTEGER NOT NULL DEFAULT 0,
  quantity_dropped   INTEGER NOT NULL DEFAULT 0,
  value_isk          REAL    NOT NULL DEFAULT 0,
  PRIMARY KEY (character_id, killmail_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_killmail_items_character_id_killmail_id ON killmail_items(character_id, killmail_id);
