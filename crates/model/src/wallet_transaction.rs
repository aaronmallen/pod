//! Domain model for wallet transactions.

use validator::Validate;

#[derive(Clone, Debug, Validate)]
pub struct Model {
  pub character_id: i64,
  pub transaction_id: i64,
  pub type_id: i32,
  pub quantity: i32,
  pub unit_price: f64,
  pub is_buy: bool,
  #[validate(length(min = 1))]
  pub date: String,
  pub location_id: i64,
  pub client_id: i64,
}
