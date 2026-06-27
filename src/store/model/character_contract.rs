use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub acceptor_id: Option<i64>,
  #[getset(get = "pub")]
  pub acceptor_name: Option<String>,
  #[getset(get_copy = "pub")]
  pub assignee_id: Option<i64>,
  #[getset(get = "pub")]
  pub assignee_name: Option<String>,
  #[getset(get = "pub")]
  pub availability: Option<String>,
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub collateral: Option<f64>,
  #[getset(get_copy = "pub")]
  pub contract_id: i64,
  #[getset(get = "pub")]
  pub date_accepted: Option<String>,
  #[getset(get = "pub")]
  pub date_completed: Option<String>,
  #[getset(get = "pub")]
  pub date_expired: Option<String>,
  #[getset(get = "pub")]
  pub date_issued: String,
  #[getset(get_copy = "pub")]
  pub days_to_complete: Option<i64>,
  #[getset(get_copy = "pub")]
  pub end_location_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub for_corporation: bool,
  #[getset(get_copy = "pub")]
  pub issuer_corporation_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub issuer_id: i64,
  #[getset(get = "pub")]
  pub issuer_name: Option<String>,
  #[getset(get_copy = "pub")]
  pub price: Option<f64>,
  #[getset(get_copy = "pub")]
  pub reward: Option<f64>,
  #[getset(get_copy = "pub")]
  pub start_location_id: Option<i64>,
  #[getset(get = "pub")]
  pub status: String,
  #[getset(get = "pub")]
  pub title: Option<String>,
  #[getset(get = "pub")]
  pub r#type: String,
  #[getset(get_copy = "pub")]
  pub volume: Option<f64>,
}

#[derive(Clone, CopyGetters, Debug, FromRow, PartialEq)]
// Public store API exercised by unit tests; not yet wired into a production call site.
pub struct ContractEscrow {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub escrow: f64,
  #[getset(get_copy = "pub")]
  pub escrow_collateral: f64,
  #[getset(get_copy = "pub")]
  pub escrow_price: f64,
}
