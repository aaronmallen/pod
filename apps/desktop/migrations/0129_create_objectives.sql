-- Standing Orders storage: account-scoped durable objectives, the pilots assigned to each, and the
-- day-scoped links tying an objective to Captain's Log items.
--
-- `objectives` is account-scoped with a surrogate id and no character_id on the parent, mirroring the
-- captains_log account model. `objective_pilots` assigns owned characters to an objective; both foreign keys
-- cascade so removing an objective or a character clears the pairing. `objective_links` is a polymorphic,
-- day-scoped link following the entity_tags shape (source_kind/source_ref instead of a typed foreign key)
-- plus a `date` column, so one table serves every Captain's Log source. A link stores the item's stable
-- identity, never a captains_log or captains_log_answer row id, so renaming, reordering, or removing prompt
-- questions never orphans or moves a link. See ADR-0046 for the source_ref encoding per source_kind.

CREATE TABLE IF NOT EXISTS objectives (
  id           INTEGER NOT NULL PRIMARY KEY,
  title        TEXT    NOT NULL,
  why          TEXT,
  target       TEXT,
  horizon      TEXT,
  accent       TEXT    NOT NULL,
  status       TEXT    NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'complete', 'cancelled')),
  created_at   TEXT    NOT NULL,
  completed_at TEXT,
  cancelled_at TEXT
);

CREATE TABLE IF NOT EXISTS objective_pilots (
  objective_id INTEGER NOT NULL REFERENCES objectives(id) ON DELETE CASCADE,
  character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
  PRIMARY KEY (objective_id, character_id)
);

CREATE INDEX IF NOT EXISTS idx_objective_pilots_character ON objective_pilots(character_id);

CREATE TABLE IF NOT EXISTS objective_links (
  objective_id INTEGER NOT NULL REFERENCES objectives(id) ON DELETE CASCADE,
  date         TEXT    NOT NULL,
  source_kind  TEXT    NOT NULL,
  source_ref   TEXT    NOT NULL,
  PRIMARY KEY (objective_id, date, source_kind, source_ref)
);

CREATE INDEX IF NOT EXISTS idx_objective_links_day ON objective_links(date);
