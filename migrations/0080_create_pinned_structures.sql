-- Player-pinned structures: public or allied citadels a character personally has docking access to but
-- which no managed corporation owns, so they never reach the corp-sync `structures` table. Picking one in
-- the industry planner persists it here, making it a permanent, selectable facility independent of corp
-- ownership. Mirrors the parallel `inaccessible_structures` marker table; keeps the corp-sync
-- `upsert_structure` path untouched. Carries the minimal resolved shape (name, system, type) needed to
-- render and cost a facility, with the same FK references the `structures` table uses.
CREATE TABLE IF NOT EXISTS pinned_structures (
  id              INTEGER PRIMARY KEY NOT NULL,
  solar_system_id INTEGER NOT NULL REFERENCES solar_systems(id),
  type_id         INTEGER REFERENCES item_types(id),
  name            TEXT    NOT NULL,
  pinned_at       TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_pinned_structures_solar_system_id ON pinned_structures(solar_system_id);
CREATE INDEX IF NOT EXISTS idx_pinned_structures_type_id         ON pinned_structures(type_id);
