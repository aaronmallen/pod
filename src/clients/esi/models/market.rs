use serde::Deserialize;

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
}
