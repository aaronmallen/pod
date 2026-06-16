use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub corporation_id: i64,
  #[getset(get_copy = "pub")]
  pub from_id: i64,
  #[getset(get = "pub")]
  pub from_name: String,
  #[getset(get = "pub")]
  pub from_type: String,
  #[getset(get_copy = "pub")]
  pub standing: f64,
}
