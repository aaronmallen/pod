use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  pub chunk_arrival_time: Option<String>,
  #[getset(get_copy = "pub")]
  pub corporation_id: i64,
  #[getset(get = "pub")]
  pub extraction_start_time: Option<String>,
  #[getset(get_copy = "pub")]
  pub moon_id: i64,
  #[getset(get = "pub")]
  pub moon_name: Option<String>,
  #[getset(get = "pub")]
  pub natural_decay_time: Option<String>,
  #[getset(get_copy = "pub")]
  pub security_status: Option<f64>,
  #[getset(get_copy = "pub")]
  pub solar_system_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub structure_id: i64,
}
