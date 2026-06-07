use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AllianceInfo {
  pub creator_corporation_id: i64,
  pub creator_id: i64,
  pub date_founded: String,
  #[serde(default)]
  pub executor_corporation_id: Option<i64>,
  #[serde(default)]
  pub faction_id: Option<i64>,
  pub name: String,
  pub ticker: String,
}
