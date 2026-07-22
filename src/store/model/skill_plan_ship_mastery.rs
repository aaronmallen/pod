use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  plan_id: i64,
  #[getset(get_copy = "pub")]
  ship_type_id: i64,
  #[getset(get_copy = "pub")]
  tier: i64,
}
