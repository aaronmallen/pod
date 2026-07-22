use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  id: i64,
  #[getset(get = "pub")]
  name: String,
  #[getset(get_copy = "pub")]
  product_type_id: i64,
  #[getset(get_copy = "pub")]
  root_facility_system: Option<i64>,
  #[getset(get_copy = "pub")]
  runs: i64,
  #[getset(get = "pub")]
  saved_at: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanSegment {
  pub clone_id: Option<i64>,
  pub pilot_id: Option<i64>,
  pub runs: i64,
  pub segment_index: i64,
  pub type_id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanTree {
  pub product_type_id: i64,
  pub root_facility_system: Option<i64>,
  pub runs: i64,
  pub types: Vec<PlanType>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanType {
  pub built: bool,
  pub facility_structure: Option<i64>,
  pub facility_system: Option<i64>,
  pub me: i64,
  pub te: i64,
  pub type_id: i64,
  pub use_stock: bool,
}
