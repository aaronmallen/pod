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
  /// The icon resolved off the render path (the loader stats the filesystem once and caches the outcome here so
  /// the implant grid never stats in `view`). Defaults to [`IconResolution::Missing`] for rows read straight from
  /// SQL; the repo overwrites it after resolving against the type id.
  #[getset(get = "pub")]
  #[sqlx(skip)]
  pub resolved_icon: IconResolution,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
}
