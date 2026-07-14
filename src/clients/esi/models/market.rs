use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CorporationMarketOrder {
  pub duration: i64,
  #[serde(default)]
  pub escrow: f64,
  #[serde(default)]
  pub is_buy_order: bool,
  pub issued: String,
  pub location_id: i64,
  pub order_id: i64,
  pub price: f64,
  pub range: String,
  pub region_id: i64,
  pub type_id: i64,
  pub volume_remain: i64,
  pub volume_total: i64,
}

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

#[derive(Debug, Default, Deserialize)]
pub struct RegionOrder {
  #[serde(default)]
  pub duration: i64,
  #[serde(default)]
  pub is_buy_order: bool,
  #[serde(default)]
  pub issued: String,
  pub location_id: i64,
  #[serde(default)]
  pub min_volume: i64,
  #[serde(default)]
  pub order_id: i64,
  pub price: f64,
  #[serde(default)]
  pub range: String,
  #[serde(default)]
  pub system_id: i64,
  pub type_id: i64,
  #[serde(default)]
  pub volume_remain: i64,
}
