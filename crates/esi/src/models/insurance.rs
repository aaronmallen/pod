//! Insurance ESI response models.

use serde::{Deserialize, Serialize};

/// Insurance prices for a ship type.
#[derive(Debug, Deserialize, Serialize)]
pub struct InsurancePrice {
  pub levels: Vec<InsuranceLevel>,
  pub type_id: i32,
}

/// A specific insurance tier and its cost/payout.
#[derive(Debug, Deserialize, Serialize)]
pub struct InsuranceLevel {
  pub cost: f64,
  pub name: String,
  pub payout: f64,
}
