use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct MarketHistory {
  pub average: f64,
  pub date: String,
  pub highest: f64,
  pub lowest: f64,
  pub order_count: i64,
  pub volume: i64,
}

#[derive(Debug, Deserialize)]
pub struct MarketPrice {
  #[serde(default)]
  pub adjusted_price: Option<f64>,
  #[serde(default)]
  pub average_price: Option<f64>,
  pub type_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct RegionOrder {
  #[serde(default)]
  pub is_buy_order: bool,
  pub location_id: i64,
  pub price: f64,
  // Deserialized off the ESI payload; read only by this module's tests until the live-market MCP tool lands.
  #[cfg_attr(not(test), expect(dead_code))]
  pub type_id: i64,
  #[serde(default)]
  pub volume_remain: i64,
}
