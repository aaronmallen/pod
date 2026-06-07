use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get_copy = "pub")]
  pub location_id: Option<i64>,
  #[getset(get = "pub")]
  pub name: String,
}
