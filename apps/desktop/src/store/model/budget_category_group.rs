use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, Debug, PartialEq)]
pub struct NewGroup {
  pub name: String,
  pub position: i64,
}

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
  #[getset(get = "pub")]
  pub updated_at: String,
}
