use sqlx::FromRow;

#[derive(Clone, Debug, Default, Eq, FromRow, PartialEq)]
pub struct AttendeeTally {
  pub accepted: i64,
  pub declined: i64,
  pub invited: i64,
  pub tentative: i64,
}
