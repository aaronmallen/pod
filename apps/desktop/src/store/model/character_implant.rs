use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, Copy, CopyGetters, Debug, Eq, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub attribute_id: i64,
  #[getset(get_copy = "pub")]
  pub bonus: i64,
  #[getset(get_copy = "pub")]
  pub character_id: i64,
}
