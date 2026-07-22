use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub orbit_index: Option<i64>,
  #[getset(get_copy = "pub")]
  pub planet_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub position_x: f64,
  #[getset(get_copy = "pub")]
  pub position_y: f64,
  #[getset(get_copy = "pub")]
  pub position_z: f64,
  #[getset(get_copy = "pub")]
  pub radius: Option<f64>,
  #[getset(get_copy = "pub")]
  pub solar_system_id: i64,
  #[getset(get_copy = "pub")]
  pub type_id: Option<i64>,
}
