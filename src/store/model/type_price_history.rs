use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub close: f64,
  #[getset(get = "pub")]
  pub date: String,
  #[getset(get_copy = "pub")]
  pub high: f64,
  #[getset(get_copy = "pub")]
  pub low: f64,
  #[getset(get_copy = "pub")]
  pub open: f64,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
}
