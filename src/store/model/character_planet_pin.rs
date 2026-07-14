use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
#[allow(dead_code)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub cycle_time: Option<i64>,
  #[getset(get = "pub")]
  pub expiry_time: Option<String>,
  #[getset(get_copy = "pub")]
  pub head_radius: Option<f64>,
  #[getset(get = "pub")]
  pub install_time: Option<String>,
  #[getset(get = "pub")]
  pub last_cycle_start: Option<String>,
  #[getset(get_copy = "pub")]
  pub latitude: f64,
  #[getset(get_copy = "pub")]
  pub longitude: f64,
  #[getset(get_copy = "pub")]
  pub pin_id: i64,
  #[getset(get_copy = "pub")]
  pub planet_id: i64,
  #[getset(get_copy = "pub")]
  pub product_type_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub qty_per_cycle: Option<i64>,
  #[getset(get_copy = "pub")]
  pub schematic_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
}
