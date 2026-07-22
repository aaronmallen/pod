use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub flag: i64,
  #[getset(get_copy = "pub")]
  pub killmail_id: i64,
  #[getset(get_copy = "pub")]
  pub ordinal: i64,
  #[getset(get_copy = "pub")]
  pub quantity_destroyed: i64,
  #[getset(get_copy = "pub")]
  pub quantity_dropped: i64,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
  #[getset(get_copy = "pub")]
  pub value_isk: f64,
}
