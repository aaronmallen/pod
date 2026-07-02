-- Make facility intel self-contained. Intel is keep-forever user data (the rigs a user reports fitted to a
-- facility), but rendering it in Settings previously required the facility to still be accessible: a tombstoned
-- or lost-access structure dropped out of the picker and its intel silently vanished from the UI. Denormalize the
-- facility's display identity onto the intel row so a card can always render from its own data, independent of
-- accessibility. All three columns are nullable: a row whose facility no match can be found for keeps NULLs and
-- renders as `#<facility_id>`.
ALTER TABLE facility_intel ADD COLUMN name            TEXT;
ALTER TABLE facility_intel ADD COLUMN solar_system_id INTEGER;
ALTER TABLE facility_intel ADD COLUMN type_id         INTEGER;

-- Backfill existing rows from whatever source still knows the facility, preferring the richest: a player pin, then
-- a corp-synced structure, then an NPC station (stations key their system as `system_id`). A facility_id with no
-- match in any source keeps its NULLs.
UPDATE facility_intel SET
  name = COALESCE(
    (SELECT p.name FROM pinned_structures p WHERE p.id = facility_intel.facility_id),
    (SELECT s.name FROM structures s        WHERE s.id = facility_intel.facility_id),
    (SELECT st.name FROM stations st        WHERE st.id = facility_intel.facility_id)
  ),
  solar_system_id = COALESCE(
    (SELECT p.solar_system_id FROM pinned_structures p WHERE p.id = facility_intel.facility_id),
    (SELECT s.solar_system_id FROM structures s        WHERE s.id = facility_intel.facility_id),
    (SELECT st.system_id FROM stations st              WHERE st.id = facility_intel.facility_id)
  ),
  type_id = COALESCE(
    (SELECT p.type_id FROM pinned_structures p WHERE p.id = facility_intel.facility_id),
    (SELECT s.type_id FROM structures s        WHERE s.id = facility_intel.facility_id),
    (SELECT st.type_id FROM stations st        WHERE st.id = facility_intel.facility_id)
  );
