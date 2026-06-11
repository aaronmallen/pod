use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub container_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub corporation_id: i64,
  #[getset(get_copy = "pub")]
  pub depth: i64,
  #[getset(get_copy = "pub")]
  pub is_blueprint_copy: Option<bool>,
  #[getset(get_copy = "pub")]
  pub is_container: bool,
  #[getset(get_copy = "pub")]
  pub is_singleton: bool,
  #[getset(get_copy = "pub")]
  pub item_id: i64,
  #[getset(get = "pub")]
  pub location_flag: String,
  #[getset(get_copy = "pub")]
  pub location_id: i64,
  #[getset(get = "pub")]
  pub location_type: String,
  #[getset(get = "pub")]
  pub name: Option<String>,
  #[getset(get_copy = "pub")]
  pub quantity: i64,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
}
