use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::store::images::IconResolution;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub clone_id: Option<i64>,
  #[getset(get = "pub")]
  pub icon: Option<String>,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get = "pub")]
  #[sqlx(skip)]
  pub resolved_icon: IconResolution,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
}
