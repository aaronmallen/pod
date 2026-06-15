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
