use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Clone, Debug, FromRow, PartialEq)]
pub struct Model {
  pub alliance_tax_rate: Option<f64>,
  pub allow_access_with_standings: bool,
  pub allow_alliance_access: bool,
  pub bad_standing_tax_rate: Option<f64>,
  pub corporation_id: i64,
  pub corporation_tax_rate: Option<f64>,
  pub excellent_standing_tax_rate: Option<f64>,
  pub good_standing_tax_rate: Option<f64>,
  pub neutral_standing_tax_rate: Option<f64>,
  pub office_id: i64,
  pub planet_id: Option<i64>,
  pub reinforce_exit_end: i64,
  pub reinforce_exit_start: i64,
  pub standing_level: String,
  pub synced_at: String,
  pub system_id: i64,
  pub terrible_standing_tax_rate: Option<f64>,
}
