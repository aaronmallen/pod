use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, Copy, CopyGetters, Debug, Eq, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get_copy = "pub")]
  pub stockpile_id: i64,
  #[getset(get_copy = "pub")]
  pub target_quantity: i64,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
}
