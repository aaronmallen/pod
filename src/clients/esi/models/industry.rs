use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CostIndex {
  pub activity: String,
  pub cost_index: f64,
}

#[derive(Debug, Deserialize)]
pub struct SystemCostIndices {
  pub cost_indices: Vec<CostIndex>,
  pub solar_system_id: i64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct IndustryJob {
  pub activity_id: i32,
  pub blueprint_id: i64,
  pub blueprint_location_id: i64,
  pub blueprint_type_id: i32,
  #[serde(default)]
  pub completed_character_id: Option<i64>,
  #[serde(default)]
  pub completed_date: Option<String>,
  #[serde(default)]
  pub cost: Option<f64>,
  pub duration: i32,
  pub end_date: String,
  pub facility_id: i64,
  pub installer_id: i64,
  pub job_id: i64,
  #[serde(default)]
  pub licensed_runs: Option<i32>,
  pub output_location_id: i64,
  #[serde(default)]
  pub pause_date: Option<String>,
  #[serde(default)]
  pub probability: Option<f64>,
  #[serde(default)]
  pub product_type_id: Option<i32>,
  pub runs: i32,
  pub start_date: String,
  #[serde(default)]
  pub station_id: Option<i64>,
  pub status: String,
  #[serde(default)]
  pub successful_runs: Option<i32>,
}
