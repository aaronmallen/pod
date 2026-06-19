use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub amount: f64,
  #[getset(get = "pub")]
  pub by_date: Option<String>,
  #[getset(get_copy = "pub")]
  pub category_id: i64,
  #[getset(get = "pub")]
  pub kind: String,
}
