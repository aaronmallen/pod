use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub home_location_id: i64,
  #[getset(get = "pub")]
  pub home_location_name: Option<String>,
  #[getset(get = "pub")]
  pub home_location_type: String,
  #[getset(get = "pub")]
  pub last_clone_jump_date: Option<String>,
  #[getset(get = "pub")]
  pub last_station_change_date: Option<String>,
}
