ALTER TABLE character_killmails ADD COLUMN value_destroyed_isk REAL NOT NULL DEFAULT 0;
ALTER TABLE character_killmails ADD COLUMN value_source TEXT NOT NULL DEFAULT 'local';
ALTER TABLE character_killmails ADD COLUMN value_recheck_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE character_killmails ADD COLUMN value_final INTEGER NOT NULL DEFAULT 0;
CREATE INDEX IF NOT EXISTS idx_character_killmails_recheck ON character_killmails(value_source, value_final);
