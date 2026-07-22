use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[allow(dead_code)]
#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  pub first_seen: String,
  #[getset(get_copy = "pub")]
  pub is_buy_order: bool,
  #[getset(get_copy = "pub")]
  pub is_corporation: bool,
  #[getset(get = "pub")]
  pub issued: String,
  #[getset(get = "pub")]
  pub last_seen: String,
  #[getset(get_copy = "pub")]
  pub location_id: i64,
  #[getset(get_copy = "pub")]
  pub order_id: i64,
  #[getset(get_copy = "pub")]
  pub owner_id: i64,
  #[getset(get_copy = "pub")]
  pub price: f64,
  #[getset(get_copy = "pub")]
  pub region_id: i64,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
}
