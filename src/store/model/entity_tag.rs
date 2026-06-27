use getset::{CopyGetters, Getters};
use sqlx::FromRow;

// Asset membership entity-type, consumed by the asset assign/unassign and inventory tagging tasks.
pub const ENTITY_TYPE_ASSET: &str = "asset";

pub const ENTITY_TYPE_CHARACTER: &str = "character";

pub const ENTITY_TYPE_CORPORATION: &str = "corporation";

pub const TAG_SCOPE_ASSET: &str = "asset";

pub const TAG_SCOPE_ENTITY: &str = "entity";

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  entity_id: i64,
  #[getset(get = "pub")]
  entity_type: String,
  #[getset(get_copy = "pub")]
  tag_id: i64,
}
