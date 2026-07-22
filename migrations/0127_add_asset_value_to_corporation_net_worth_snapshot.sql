-- Give owned-corporation net-worth snapshots an asset component.
--
-- corporation_net_worth_snapshot (0043) recorded liquid cash only, so a corp converting ISK into
-- assets read as a net-worth dip in the combined series. Add a nullable asset_value REAL so
-- record_today can store liquid + asset_value going forward. Assets are not historized, so existing
-- rows stay NULL (liquid-only) and are never repriced; the old dips age out of the window naturally.

ALTER TABLE corporation_net_worth_snapshot ADD COLUMN asset_value REAL;
