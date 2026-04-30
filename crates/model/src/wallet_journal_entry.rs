//! Domain model for wallet journal entries.

use validator::Validate;

#[derive(Clone, Debug, Validate)]
pub struct Model {
  pub character_id: i64,
  pub entry_id: i64,
  #[validate(length(min = 1))]
  pub ref_type: String,
  pub amount: Option<f64>,
  pub balance: Option<f64>,
  #[validate(length(min = 1))]
  pub date: String,
  pub description: String,
  pub first_party_id: Option<i64>,
  pub second_party_id: Option<i64>,
}
