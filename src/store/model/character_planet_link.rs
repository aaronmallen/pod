use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub destination_pin_id: i64,
  #[getset(get_copy = "pub")]
  pub link_level: i64,
  #[getset(get_copy = "pub")]
  pub planet_id: i64,
  #[getset(get_copy = "pub")]
  pub source_pin_id: i64,
}
