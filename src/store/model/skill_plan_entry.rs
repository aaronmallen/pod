use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  id: i64,
  #[getset(get_copy = "pub")]
  is_auto: i64,
  #[getset(get = "pub")]
  note: String,
  #[getset(get_copy = "pub")]
  plan_id: i64,
  #[getset(get_copy = "pub")]
  position: i64,
  #[getset(get = "pub")]
  priority: String,
  #[getset(get_copy = "pub")]
  skill_id: i64,
  #[getset(get_copy = "pub")]
  to_level: i64,
}
