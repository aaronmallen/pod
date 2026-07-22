use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub corporation_id: i64,
  #[getset(get_copy = "pub")]
  pub item_id: i64,
  #[getset(get = "pub")]
  pub location_flag: String,
  #[getset(get_copy = "pub")]
  pub location_id: i64,
  #[getset(get_copy = "pub")]
  pub material_efficiency: i64,
  #[getset(get_copy = "pub")]
  pub quantity: i64,
  #[getset(get_copy = "pub")]
  pub runs: i64,
  #[getset(get_copy = "pub")]
  pub time_efficiency: i64,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
}
