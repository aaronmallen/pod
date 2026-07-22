use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct DynamicItem {
  #[serde(default)]
  pub dogma_attributes: Vec<DynamicItemAttribute>,
  pub mutator_type_id: i64,
  pub source_type_id: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DynamicItemAttribute {
  pub attribute_id: i32,
  pub value: f64,
}
