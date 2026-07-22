CREATE TABLE IF NOT EXISTS industry_cost_indices (
  solar_system_id              INTEGER NOT NULL PRIMARY KEY,
  manufacturing                REAL,
  researching_time_efficiency  REAL,
  researching_material_efficiency REAL,
  copying                      REAL,
  invention                    REAL,
  reaction                     REAL
);
CREATE INDEX IF NOT EXISTS idx_industry_cost_indices_solar_system_id
  ON industry_cost_indices(solar_system_id);
