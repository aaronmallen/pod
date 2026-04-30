//! Market ESI response models.

use serde::{Deserialize, Serialize};

/// A market group.
#[derive(Debug, Deserialize, Serialize)]
pub struct MarketGroup {
  pub description: String,
  pub market_group_id: i32,
  pub name: String,
  pub parent_group_id: Option<i32>,
  pub types: Vec<i32>,
}

/// Historical market data for a type in a region.
#[derive(Debug, Deserialize, Serialize)]
pub struct MarketHistory {
  pub average: f64,
  pub date: String,
  pub highest: f64,
  pub lowest: f64,
  pub order_count: i64,
  pub volume: i64,
}

/// A market order.
#[derive(Debug, Deserialize, Serialize)]
pub struct MarketOrder {
  pub duration: i32,
  pub is_buy_order: bool,
  pub issued: String,
  pub location_id: i64,
  pub min_volume: i32,
  pub order_id: i64,
  pub price: f64,
  pub range: String,
  pub system_id: i64,
  pub type_id: i32,
  pub volume_remain: i32,
  pub volume_total: i32,
}

/// Current market price for a type.
#[derive(Debug, Deserialize, Serialize)]
pub struct MarketPrice {
  pub adjusted_price: Option<f64>,
  pub average_price: Option<f64>,
  pub type_id: i32,
}
