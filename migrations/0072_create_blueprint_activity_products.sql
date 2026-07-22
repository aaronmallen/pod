CREATE TABLE IF NOT EXISTS blueprint_activity_products (
  blueprint_type_id INTEGER NOT NULL,
  activity_id       INTEGER NOT NULL,
  product_type_id   INTEGER NOT NULL,
  quantity          INTEGER NOT NULL,
  PRIMARY KEY (blueprint_type_id, activity_id, product_type_id)
);
CREATE INDEX IF NOT EXISTS idx_blueprint_activity_products_product_type_id ON blueprint_activity_products(product_type_id);
