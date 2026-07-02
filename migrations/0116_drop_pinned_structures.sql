-- Remove structure pinning entirely. Pins let a player-curated structure persist in the facility picker forever
-- with no access re-verification, so a user who lost docking access (ACL change or the structure destroyed) was
-- still offered it as buildable. The picker now sources only from ESI-backed data (NPC stations, tombstone-filtered
-- corp structures, live search); keep-forever facility data lives in `facility_intel`, whose snapshot columns were
-- backfilled from this table in 0115 before it is dropped here.
DROP INDEX IF EXISTS idx_pinned_structures_solar_system_id;
DROP INDEX IF EXISTS idx_pinned_structures_type_id;
DROP TABLE IF EXISTS pinned_structures;
