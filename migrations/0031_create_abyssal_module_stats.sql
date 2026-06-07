CREATE TABLE IF NOT EXISTS abyssal_module_stats (
  abyssal_type_id INTEGER NOT NULL,
  attribute_id    INTEGER NOT NULL,
  min_mult        REAL    NOT NULL,
  max_mult        REAL    NOT NULL,
  PRIMARY KEY (abyssal_type_id, attribute_id)
);
CREATE INDEX IF NOT EXISTS idx_abyssal_module_stats_abyssal_type_id ON abyssal_module_stats(abyssal_type_id);
