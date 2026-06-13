use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub activity_id: i64,
  #[getset(get_copy = "pub")]
  pub blueprint_id: i64,
  #[getset(get_copy = "pub")]
  pub blueprint_location_id: i64,
  #[getset(get_copy = "pub")]
  pub blueprint_type_id: i64,
  #[getset(get_copy = "pub")]
  pub completed_character_id: Option<i64>,
  #[getset(get = "pub")]
  pub completed_date: Option<String>,
  #[getset(get_copy = "pub")]
  pub corporation_id: i64,
  #[getset(get_copy = "pub")]
  pub cost: Option<f64>,
  #[getset(get_copy = "pub")]
  pub duration: i64,
  #[getset(get = "pub")]
  pub end_date: String,
  #[getset(get_copy = "pub")]
  pub facility_id: i64,
  #[getset(get_copy = "pub")]
  pub installer_id: i64,
  #[getset(get_copy = "pub")]
  pub job_id: i64,
  #[getset(get_copy = "pub")]
  pub licensed_runs: Option<i64>,
  #[getset(get_copy = "pub")]
  pub output_location_id: i64,
  #[getset(get = "pub")]
  pub pause_date: Option<String>,
  #[getset(get_copy = "pub")]
  pub probability: Option<f64>,
  #[getset(get_copy = "pub")]
  pub product_type_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub runs: i64,
  #[getset(get = "pub")]
  pub start_date: String,
  #[getset(get_copy = "pub")]
  pub station_id: Option<i64>,
  #[getset(get = "pub")]
  pub status: String,
  #[getset(get_copy = "pub")]
  pub successful_runs: Option<i64>,
}
