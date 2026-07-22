CREATE TABLE IF NOT EXISTS industry_plans (
  id                  INTEGER PRIMARY KEY,
  name                TEXT    NOT NULL DEFAULT 'Untitled plan',
  product_type_id     INTEGER NOT NULL,
  runs                INTEGER NOT NULL,
  root_facility_system INTEGER,
  saved_at            TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_industry_plans_saved ON industry_plans(saved_at);
