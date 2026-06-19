use getset::{CopyGetters, Getters};
use sqlx::FromRow;

pub const SOURCE_ESI: &str = "esi";
pub const SOURCE_ZKILL: &str = "zkill";

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub adjusted_price: Option<f64>,
  #[getset(get_copy = "pub")]
  pub average_price: Option<f64>,
  #[getset(get = "pub")]
  pub source: String,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
}

impl Model {
  pub fn esi(type_id: i64, adjusted_price: Option<f64>, average_price: Option<f64>) -> Self {
    Self {
      adjusted_price,
      average_price,
      source: SOURCE_ESI.to_string(),
      type_id,
    }
  }

  pub fn zkill(type_id: i64, average_price: f64) -> Self {
    Self {
      adjusted_price: None,
      average_price: Some(average_price),
      source: SOURCE_ZKILL.to_string(),
      type_id,
    }
  }
}
