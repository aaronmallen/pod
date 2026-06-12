#![allow(dead_code)]

use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub agent_type_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub corporation_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub division_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get_copy = "pub")]
  pub is_locator: i32,
  #[getset(get_copy = "pub")]
  pub level: Option<i64>,
  #[getset(get_copy = "pub")]
  pub location_id: Option<i64>,
  #[getset(get = "pub")]
  pub name: String,
}
