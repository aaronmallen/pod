use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub jump_clone_id: i64,
  #[getset(get_copy = "pub")]
  pub location_id: i64,
  #[getset(get = "pub")]
  pub location_name: Option<String>,
  #[getset(get = "pub")]
  pub location_type: String,
  #[getset(get = "pub")]
  pub name: Option<String>,
}
