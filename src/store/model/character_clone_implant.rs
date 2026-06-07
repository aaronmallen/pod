use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub clone_id: Option<i64>,
  #[getset(get = "pub")]
  pub icon: Option<String>,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
}
