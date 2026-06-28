use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::store::model::BudgetScope;

#[derive(Clone, Debug, PartialEq)]
pub struct NewGroup {
  pub name: String,
  pub position: i64,
  pub scope: BudgetScope,
}

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  pub created_at: String,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub position: i64,
  #[getset(get_copy = "pub")]
  pub scope_id: Option<i64>,
  #[getset(get = "pub")]
  pub scope_kind: String,
  #[getset(get = "pub")]
  pub updated_at: String,
}
