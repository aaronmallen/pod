use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  color: Option<String>,
  #[getset(get_copy = "pub")]
  created_at: i64,
  #[getset(get = "pub")]
  description: Option<String>,
  #[getset(get_copy = "pub")]
  id: i64,
  #[getset(get = "pub")]
  name: String,
  #[getset(get_copy = "pub")]
  position: i64,
  #[getset(get_copy = "pub")]
  updated_at: i64,
}
