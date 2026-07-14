use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub amount: i64,
  #[getset(get_copy = "pub")]
  pub pin_id: i64,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
}
