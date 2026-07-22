use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub content_type_id: i64,
  #[getset(get_copy = "pub")]
  pub destination_pin_id: i64,
  #[getset(get_copy = "pub")]
  pub planet_id: i64,
  #[getset(get_copy = "pub")]
  pub quantity: f64,
  #[getset(get_copy = "pub")]
  pub route_id: i64,
  #[getset(get_copy = "pub")]
  pub source_pin_id: i64,
}
