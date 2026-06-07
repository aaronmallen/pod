use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, Copy, CopyGetters, Debug, Eq, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub certificate_id: i64,
  #[getset(get_copy = "pub")]
  pub ship_type_id: i64,
  #[getset(get_copy = "pub")]
  pub tier: i64,
}
