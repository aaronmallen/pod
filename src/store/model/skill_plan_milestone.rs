use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  after_entry_id: Option<i64>,
  /// Whether this milestone's `base_*` attributes are auto-computed by the remap optimizer rather than
  /// set manually.
  #[getset(get_copy = "pub")]
  auto_remap: bool,
  /// `None` when no remap is assigned at this milestone; when set, all five `base_*` fields are
  /// populated together as the target attribute set.
  #[getset(get_copy = "pub")]
  base_charisma: Option<i64>,
  #[getset(get_copy = "pub")]
  base_intelligence: Option<i64>,
  #[getset(get_copy = "pub")]
  base_memory: Option<i64>,
  #[getset(get_copy = "pub")]
  base_perception: Option<i64>,
  #[getset(get_copy = "pub")]
  base_willpower: Option<i64>,
  #[getset(get_copy = "pub")]
  id: i64,
  #[getset(get = "pub")]
  name: String,
  #[getset(get_copy = "pub")]
  plan_id: i64,
  #[getset(get_copy = "pub")]
  position: i64,
}
