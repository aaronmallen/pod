use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  pub created_at: String,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub position: i64,
  #[getset(get_copy = "pub")]
  pub scope_id: Option<i64>,
  #[getset(get = "pub")]
  pub scope_kind: String,
  #[getset(get = "pub")]
  pub updated_at: String,
}
