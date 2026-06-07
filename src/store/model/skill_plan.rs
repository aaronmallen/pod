use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  character_id: i64,
  #[getset(get = "pub")]
  created_at: String,
  #[getset(get_copy = "pub")]
  id: i64,
  #[getset(get = "pub")]
  implant_set: String,
  #[getset(get = "pub")]
  name: String,
  #[getset(get = "pub")]
  sort_mode: String,
  #[getset(get = "pub")]
  updated_at: String,
}
