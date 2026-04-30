//! Loyalty store ESI response models.

use serde::{Deserialize, Serialize};

/// A loyalty store offer.
#[derive(Debug, Deserialize, Serialize)]
pub struct LoyaltyOffer {
  pub ak_cost: Option<i32>,
  pub isk_cost: i64,
  pub lp_cost: i32,
  pub offer_id: i64,
  pub quantity: i32,
  pub required_items: Vec<RequiredItem>,
  pub type_id: i32,
}

/// A required item for a loyalty offer.
#[derive(Debug, Deserialize, Serialize)]
pub struct RequiredItem {
  pub quantity: i32,
  pub type_id: i32,
}
