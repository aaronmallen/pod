use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Faction {
  #[serde(default)]
  pub corporation_id: Option<i64>,
  pub description: String,
  pub faction_id: i64,
  pub is_unique: bool,
  #[serde(default)]
  pub militia_corporation_id: Option<i64>,
  pub name: String,
  pub size_factor: f64,
  #[serde(default)]
  pub solar_system_id: Option<i64>,
  pub station_count: i32,
  pub station_system_count: i32,
}
