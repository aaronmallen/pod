ALTER TABLE credentials ADD COLUMN needs_reauth INTEGER NOT NULL DEFAULT 0;
ALTER TABLE credentials ADD COLUMN last_checked_at INTEGER;
