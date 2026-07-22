use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub contract_id: i64,
  #[getset(get_copy = "pub")]
  pub corporation_id: i64,
  #[getset(get_copy = "pub")]
  pub is_included: bool,
  #[getset(get_copy = "pub")]
  pub is_singleton: bool,
  #[getset(get_copy = "pub")]
  pub quantity: i64,
  #[getset(get_copy = "pub")]
  pub raw_quantity: Option<i64>,
  #[getset(get_copy = "pub")]
  pub record_id: i64,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
  #[getset(get_copy = "pub")]
  pub value_isk: f64,
}
