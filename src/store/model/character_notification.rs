use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub is_read: bool,
  #[getset(get = "pub")]
  pub notif_type: String,
  #[getset(get_copy = "pub")]
  pub notification_id: i64,
  #[getset(get_copy = "pub")]
  pub sender_id: Option<i64>,
  #[getset(get = "pub")]
  pub sender_type: Option<String>,
  #[getset(get = "pub")]
  pub synced_at: String,
  #[getset(get = "pub")]
  pub text: Option<String>,
  #[getset(get = "pub")]
  pub timestamp: String,
}
