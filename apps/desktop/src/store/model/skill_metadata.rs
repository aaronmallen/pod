use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, Copy, CopyGetters, Debug, Eq, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub primary_attribute: i64,
  #[getset(get_copy = "pub")]
  pub rank: i64,
  #[getset(get_copy = "pub")]
  pub secondary_attribute: i64,
  #[getset(get_copy = "pub")]
  pub skill_id: i64,
}
