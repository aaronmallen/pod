//! Public contract ESI response models.

use serde::{Deserialize, Serialize};

/// A public contract.
#[derive(Debug, Deserialize, Serialize)]
pub struct PublicContract {
  pub buyout: Option<f64>,
  pub collateral: Option<f64>,
  pub contract_id: i64,
  pub date_expired: String,
  pub date_issued: String,
  pub days_to_complete: Option<i32>,
  pub end_location_id: Option<i64>,
  pub for_corporation: Option<bool>,
  pub issuer_corporation_id: i64,
  pub issuer_id: i64,
  pub price: Option<f64>,
  pub reward: Option<f64>,
  pub start_location_id: Option<i64>,
  pub title: Option<String>,
  pub r#type: String,
  pub volume: Option<f64>,
}

/// A bid on a public contract.
#[derive(Debug, Deserialize, Serialize)]
pub struct ContractBid {
  pub amount: f64,
  pub bid_id: i64,
  pub bidder_id: i64,
  pub date_bid: String,
}

/// An item in a public contract.
#[derive(Debug, Deserialize, Serialize)]
pub struct ContractItem {
  pub is_included: bool,
  pub is_singleton: bool,
  pub quantity: i32,
  pub raw_quantity: Option<i32>,
  pub record_id: i64,
  pub type_id: i32,
}
