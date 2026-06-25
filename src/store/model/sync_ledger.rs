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

#[cfg(test)]
impl Model {
  pub fn new_for_test(kind: String, outcome: String, last_reason: Option<String>) -> Self {
    Self {
      kind,
      last_attempt_at: "2026-01-01T00:00:00Z".to_string(),
      last_reason,
      last_success_at: None,
      next_eligible_at: None,
      outcome,
      rows_touched: 0,
      subject_id: 0,
      subject_type: OwnerType::Character,
    }
  }
}
