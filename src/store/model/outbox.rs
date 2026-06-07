use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::store::model::OwnerType;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  attempts: i64,
  #[getset(get = "pub")]
  created_at: String,
  #[getset(get = "pub")]
  dedupe_key: Option<String>,
  #[getset(get_copy = "pub")]
  id: i64,
  #[getset(get = "pub")]
  kind: String,
  #[getset(get = "pub")]
  last_error: Option<String>,
  #[getset(get = "pub")]
  next_attempt_at: String,
  #[getset(get = "pub")]
  payload: String,
  #[getset(get = "pub")]
  status: String,
  #[getset(get_copy = "pub")]
  subject_id: i64,
  #[getset(get_copy = "pub")]
  subject_type: OwnerType,
  #[getset(get = "pub")]
  updated_at: String,
}
