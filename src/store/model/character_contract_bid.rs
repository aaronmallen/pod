use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub amount: f64,
  #[getset(get_copy = "pub")]
  pub bid_id: i64,
  #[getset(get_copy = "pub")]
  pub bidder_id: i64,
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub contract_id: i64,
  #[getset(get = "pub")]
  pub date_bid: String,
}
