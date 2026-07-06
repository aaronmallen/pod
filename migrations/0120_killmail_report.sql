-- Killmail after-action reports: one user-authored debrief per (character_id, killmail_id),
-- shared between the killmail window's Report tab and the Captain's Log combat wizard step.
--
-- Keyed (character_id, killmail_id) to match killmail_attackers/killmail_items so a report is
-- scoped to the owning character's copy of the kill and cascades when the character is removed.
-- outcome is constrained to the three KmReport pill values; happened is the required narrative,
-- while different and takeaway are optional follow-ups. No FK to character_killmails: reports may
-- outlive the retention-trimmed killmail row, and the pair is validated at write time.

CREATE TABLE IF NOT EXISTS killmail_report (
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  killmail_id  INTEGER NOT NULL,
  outcome      TEXT    NOT NULL CHECK (outcome IN ('clean', 'costly', 'learning')),
  happened     TEXT    NOT NULL,
  different    TEXT,
  takeaway     TEXT,
  created_at   TEXT    NOT NULL,
  updated_at   TEXT    NOT NULL,
  PRIMARY KEY (character_id, killmail_id)
);
CREATE INDEX IF NOT EXISTS idx_killmail_report_killmail_id ON killmail_report(killmail_id);
