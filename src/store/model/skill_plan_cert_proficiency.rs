use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  cert_id: i64,
  #[getset(get_copy = "pub")]
  level: i64,
  #[getset(get_copy = "pub")]
  plan_id: i64,
}
