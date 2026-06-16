use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub corporation_id: i64,
  #[getset(get_copy = "pub")]
  pub contact_id: i64,
  #[getset(get = "pub")]
  pub contact_name: String,
  #[getset(get = "pub")]
  pub contact_type: String,
  #[getset(get_copy = "pub")]
  pub is_blocked: bool,
  #[getset(get_copy = "pub")]
  pub is_watched: bool,
  #[getset(get = "pub")]
  pub label_ids: String,
  #[getset(get_copy = "pub")]
  pub standing: f64,
}
