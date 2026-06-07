use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  after_entry_id: Option<i64>,
  #[getset(get_copy = "pub")]
  base_charisma: i64,
  #[getset(get_copy = "pub")]
  base_intelligence: i64,
  #[getset(get_copy = "pub")]
  base_memory: i64,
  #[getset(get_copy = "pub")]
  base_perception: i64,
  #[getset(get_copy = "pub")]
  base_willpower: i64,
  #[getset(get_copy = "pub")]
  id: i64,
  #[getset(get_copy = "pub")]
  plan_id: i64,
}
