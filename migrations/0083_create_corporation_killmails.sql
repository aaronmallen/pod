-- Corp-keyed mirrors of character_killmails (incl. the 0062 value-provenance and 0074 detail
-- columns) plus the killmail detail child tables. The existing killmail_attackers/killmail_items
-- tables are FK-coupled to characters(id), so a corporation needs its own corp-keyed equivalents
-- rather than sharing those rows; hence the separate corporation_killmail_attackers /
-- corporation_killmail_items tables below.
CREATE TABLE IF NOT EXISTS corporation_killmails (
  corporation_id      INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  killmail_id         INTEGER NOT NULL,
  kill_hash           TEXT    NOT NULL,
  is_kill             INTEGER NOT NULL DEFAULT 0,
  ship_type_id        INTEGER NOT NULL,
  victim_id           INTEGER,
  victim_corp_id      INTEGER,
  victim_alliance_id  INTEGER,
  victim_damage_taken INTEGER NOT NULL DEFAULT 0,
  system_id           INTEGER NOT NULL,
  value_isk           REAL    NOT NULL DEFAULT 0,
  value_destroyed_isk REAL    NOT NULL DEFAULT 0,
  value_source        TEXT    NOT NULL DEFAULT 'local',
  value_recheck_count INTEGER NOT NULL DEFAULT 0,
  value_final         INTEGER NOT NULL DEFAULT 0,
  attacker_count      INTEGER NOT NULL DEFAULT 0,
  final_blow          INTEGER NOT NULL DEFAULT 0,
  kill_time           TEXT    NOT NULL,
  synced_at           TEXT    NOT NULL,
  PRIMARY KEY (corporation_id, killmail_id)
);
CREATE INDEX IF NOT EXISTS idx_corporation_killmails_corporation_id           ON corporation_killmails(corporation_id);
CREATE INDEX IF NOT EXISTS idx_corporation_killmails_corporation_id_kill_time ON corporation_killmails(corporation_id, kill_time);
CREATE INDEX IF NOT EXISTS idx_corporation_killmails_recheck                  ON corporation_killmails(value_source, value_final);

CREATE TABLE IF NOT EXISTS corporation_killmail_attackers (
  corporation_id          INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  killmail_id             INTEGER NOT NULL,
  ordinal                 INTEGER NOT NULL,
  attacker_character_id   INTEGER,
  attacker_corporation_id INTEGER,
  alliance_id             INTEGER,
  ship_type_id            INTEGER,
  damage_done             INTEGER NOT NULL DEFAULT 0,
  final_blow              INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (corporation_id, killmail_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_corporation_killmail_attackers_corporation_id_killmail_id ON corporation_killmail_attackers(corporation_id, killmail_id);

CREATE TABLE IF NOT EXISTS corporation_killmail_items (
  corporation_id     INTEGER NOT NULL REFERENCES corporations(id) ON DELETE CASCADE,
  killmail_id        INTEGER NOT NULL,
  ordinal            INTEGER NOT NULL,
  type_id            INTEGER NOT NULL,
  flag               INTEGER NOT NULL,
  quantity_destroyed INTEGER NOT NULL DEFAULT 0,
  quantity_dropped   INTEGER NOT NULL DEFAULT 0,
  value_isk          REAL    NOT NULL DEFAULT 0,
  PRIMARY KEY (corporation_id, killmail_id, ordinal)
);
CREATE INDEX IF NOT EXISTS idx_corporation_killmail_items_corporation_id_killmail_id ON corporation_killmail_items(corporation_id, killmail_id);
