use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Default, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  pub body: Option<String>,
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub duration_minutes: i64,
  #[getset(get_copy = "pub")]
  pub event_id: i64,
  #[getset(get = "pub")]
  pub fetched_at: String,
  #[getset(get_copy = "pub")]
  pub importance: i64,
  #[getset(get_copy = "pub")]
  pub owner_id: i64,
  #[getset(get = "pub")]
  pub owner_name: String,
  #[getset(get = "pub")]
  pub owner_type: String,
  #[getset(get = "pub")]
  pub response: String,
  #[getset(get = "pub")]
  pub timestamp: String,
  #[getset(get = "pub")]
  pub title: String,
}
