CREATE TABLE IF NOT EXISTS skill_metadata (
  skill_id            INTEGER PRIMARY KEY REFERENCES item_types(id),
  rank                INTEGER NOT NULL,
  primary_attribute   INTEGER NOT NULL,
  secondary_attribute INTEGER NOT NULL
);
