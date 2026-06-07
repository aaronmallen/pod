use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub abyssal_type_id: i64,
  #[getset(get_copy = "pub")]
  pub attribute_id: i64,
  #[getset(get_copy = "pub")]
  pub max_mult: f64,
  #[getset(get_copy = "pub")]
  pub min_mult: f64,
}

impl Model {
  pub fn new(abyssal_type_id: i64, attribute_id: i64, min_mult: f64, max_mult: f64) -> Self {
    Self {
      abyssal_type_id,
      attribute_id,
      max_mult,
      min_mult,
    }
  }
}
