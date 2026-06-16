use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get_copy = "pub")]
  pub manufacturing_index: Option<f64>,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub owner_id: Option<i64>,
  #[getset(get = "pub")]
  pub region: Option<String>,
  #[getset(get_copy = "pub")]
  pub security_status: Option<f64>,
  #[getset(get_copy = "pub")]
  pub solar_system_id: i64,
  #[getset(get_copy = "pub")]
  pub type_id: Option<i64>,
}
