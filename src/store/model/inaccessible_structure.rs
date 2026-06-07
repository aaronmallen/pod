#![allow(dead_code)]

use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::store::model::OwnerType;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub marked_at: String,
  #[getset(get_copy = "pub")]
  pub owner_id: i64,
  #[getset(get_copy = "pub")]
  pub owner_type: OwnerType,
}
