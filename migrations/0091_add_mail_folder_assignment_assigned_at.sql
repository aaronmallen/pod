ALTER TABLE mail_folder_assignment ADD COLUMN assigned_at TEXT;
UPDATE mail_folder_assignment SET assigned_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE assigned_at IS NULL;
