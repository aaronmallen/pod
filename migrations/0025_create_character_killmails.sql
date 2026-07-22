CREATE TABLE IF NOT EXISTS character_killmails (
  character_id   INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  killmail_id    INTEGER NOT NULL,
  kill_hash      TEXT    NOT NULL,
  is_kill        INTEGER NOT NULL DEFAULT 0,
  ship_type_id   INTEGER NOT NULL,
  victim_id      INTEGER,
  victim_corp_id INTEGER,
  system_id      INTEGER NOT NULL,
  value_isk      REAL    NOT NULL DEFAULT 0,
  attacker_count INTEGER NOT NULL DEFAULT 0,
  final_blow     INTEGER NOT NULL DEFAULT 0,
  kill_time      TEXT    NOT NULL,
  synced_at      TEXT    NOT NULL,
  PRIMARY KEY (character_id, killmail_id)
);
CREATE INDEX IF NOT EXISTS idx_character_killmails_character_id           ON character_killmails(character_id);
CREATE INDEX IF NOT EXISTS idx_character_killmails_character_id_kill_time ON character_killmails(character_id, kill_time);
