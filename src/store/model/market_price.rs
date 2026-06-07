use getset::CopyGetters;
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub adjusted_price: Option<f64>,
  #[getset(get_copy = "pub")]
  pub average_price: Option<f64>,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
}
