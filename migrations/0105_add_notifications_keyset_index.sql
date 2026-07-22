-- Keyset-paging support for the notification History view (epic zyrmyrlk, spec A, phase 3). The
-- center pages surfaced rows with WHERE suppressed = 0 AND (created_at, id) < (?, ?) ORDER BY
-- created_at DESC, id DESC. A partial index over only surfaced rows, leading on (created_at, id),
-- lets SQLite satisfy both the suppressed filter and the cursor scan/ordering from the index alone,
-- so deep history pages stay cheap as the table grows. Adds an index only; no columns change.

CREATE INDEX IF NOT EXISTS idx_notifications_surfaced_keyset
  ON notifications(created_at, id)
  WHERE suppressed = 0;
