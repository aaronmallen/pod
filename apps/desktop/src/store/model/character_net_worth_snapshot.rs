use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub asset_value: Option<f64>,
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get = "pub")]
  pub date: String,
  #[getset(get_copy = "pub")]
  pub escrow: Option<f64>,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get_copy = "pub")]
  pub liquid: f64,
  #[getset(get_copy = "pub")]
  pub net_worth: f64,
}

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct CombinedNetWorthPoint {
  #[getset(get_copy = "pub")]
  pub asset_value: Option<f64>,
  #[getset(get = "pub")]
  pub date: String,
  #[getset(get_copy = "pub")]
  pub escrow: Option<f64>,
  #[getset(get_copy = "pub")]
  pub liquid: Option<f64>,
  #[getset(get_copy = "pub")]
  pub net_worth: Option<f64>,
}
