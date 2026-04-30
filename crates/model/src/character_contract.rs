//! Domain model for character contracts.

use validator::Validate;

#[derive(Clone, Debug, Validate)]
pub struct Model {
  pub character_id: i64,
  pub contract_id: i64,
  pub contract_type: String,
  pub status: String,
  pub title: String,
  pub issuer_id: i64,
  pub assignee_id: i64,
  pub acceptor_id: i64,
  pub price: Option<f64>,
  pub collateral: Option<f64>,
  pub date_issued: String,
  pub date_expired: String,
  pub start_location_id: Option<i64>,
}
