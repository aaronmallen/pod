use getset::{CopyGetters, Getters};
use sqlx::FromRow;

pub const ENTITY_TYPE_CHARACTER: &str = "character";

pub const ENTITY_TYPE_CORPORATION: &str = "corporation";

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  entity_id: i64,
  #[getset(get = "pub")]
  entity_type: String,
  #[getset(get_copy = "pub")]
  tag_id: i64,
}
