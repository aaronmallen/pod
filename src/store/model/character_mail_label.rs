use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get = "pub")]
  pub color: Option<String>,
  #[getset(get_copy = "pub")]
  pub label_id: i64,
  #[getset(get = "pub")]
  pub name: String,
}
