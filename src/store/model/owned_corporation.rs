use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  alliance_id: Option<i64>,
  #[getset(get_copy = "pub")]
  authorized_by: Option<i64>,
  #[getset(get_copy = "pub")]
  ceo_id: i64,
  #[getset(get = "pub")]
  date_founded: Option<String>,
  #[getset(get = "pub")]
  description: Option<String>,
  #[getset(get_copy = "pub")]
  home_station_id: Option<i64>,
  #[getset(get_copy = "pub")]
  id: i64,
  #[getset(get_copy = "pub")]
  member_count: i32,
  #[getset(get = "pub")]
  name: String,
  #[getset(get_copy = "pub")]
  tax_rate: f64,
  #[getset(get = "pub")]
  ticker: String,
  #[getset(get = "pub")]
  url: Option<String>,
  #[getset(get_copy = "pub")]
  war_eligible: Option<bool>,
}
