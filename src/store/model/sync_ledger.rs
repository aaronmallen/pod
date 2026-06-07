#![allow(dead_code)]

use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::store::model::OwnerType;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  kind: String,
  #[getset(get = "pub")]
  last_attempt_at: String,
  #[getset(get = "pub")]
  last_reason: Option<String>,
  #[getset(get = "pub")]
  last_success_at: Option<String>,
  #[getset(get = "pub")]
  next_eligible_at: Option<String>,
  #[getset(get = "pub")]
  outcome: String,
  #[getset(get_copy = "pub")]
  rows_touched: i64,
  #[getset(get_copy = "pub")]
  subject_id: i64,
  #[getset(get_copy = "pub")]
  subject_type: OwnerType,
}
