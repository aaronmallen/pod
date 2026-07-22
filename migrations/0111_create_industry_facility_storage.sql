-- Move the default manufacturing/reactions facilities out of config (config/TOML) and into the DB,
-- and add storage for user-entered facility intel (the rigs fitted to a structure). The config copy
-- could not FK-validate against item data; the DB copy can, and survives independently of config.
--
-- `industry_default_facility` holds one row per industry activity (manufacturing = 1, reactions = 9),
-- mapping the activity to the facility chosen as its planner default. `facility_id` is the resolved
-- facility identity (NPC station, corp structure, or pinned structure), which has no single backing
-- table to FK against, so it is a bare INTEGER.
CREATE TABLE IF NOT EXISTS industry_default_facility (
  activity_id INTEGER PRIMARY KEY NOT NULL,
  facility_id INTEGER NOT NULL
);

-- `facility_intel` records the rigs a user reports fitted to a facility. EVE rigs are unordered, so
-- the three slots are cosmetic; three nullable columns model "up to three rigs" without a child
-- table, and a row may carry zero rigs. Each rig references `item_types` so a fitted rig FK-validates
-- against item data.
CREATE TABLE IF NOT EXISTS facility_intel (
  facility_id   INTEGER PRIMARY KEY NOT NULL,
  rig_1_type_id INTEGER REFERENCES item_types(id),
  rig_2_type_id INTEGER REFERENCES item_types(id),
  rig_3_type_id INTEGER REFERENCES item_types(id)
);
