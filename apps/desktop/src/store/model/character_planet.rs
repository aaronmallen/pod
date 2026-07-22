use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get = "pub")]
  pub last_update: String,
  #[getset(get_copy = "pub")]
  pub num_pins: i64,
  #[getset(get_copy = "pub")]
  pub planet_id: i64,
  #[getset(get = "pub")]
  pub planet_type: String,
  #[getset(get_copy = "pub")]
  pub solar_system_id: i64,
  #[getset(get_copy = "pub")]
  pub upgrade_level: i64,
}
