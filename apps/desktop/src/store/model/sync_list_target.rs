use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  character_id: i64,
  #[getset(get = "pub")]
  created_at: String,
  #[getset(get_copy = "pub")]
  list_id: i64,
}
