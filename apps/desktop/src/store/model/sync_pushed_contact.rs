use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  character_id: i64,
  #[getset(get = "pub")]
  created_at: String,
  #[getset(get_copy = "pub")]
  entity_id: i64,
  #[getset(get = "pub")]
  entity_type: String,
  #[getset(get_copy = "pub")]
  pushed_standing: i64,
  #[getset(get = "pub")]
  updated_at: String,
}
