use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub category_id: i64,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub ref_type: String,
  #[getset(get_copy = "pub")]
  pub scope_id: Option<i64>,
  #[getset(get = "pub")]
  pub scope_kind: String,
}
