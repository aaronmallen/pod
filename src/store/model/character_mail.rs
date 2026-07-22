use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Default, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub from_corp: bool,
  #[getset(get_copy = "pub")]
  pub from_id: i64,
  #[getset(get = "pub")]
  pub from_name: String,
  #[getset(get_copy = "pub")]
  pub from_system: bool,
  #[getset(get_copy = "pub")]
  pub has_attachment: bool,
  #[getset(get_copy = "pub")]
  pub important: bool,
  #[getset(get_copy = "pub")]
  pub is_read: bool,
  #[getset(get_copy = "pub")]
  pub mail_id: i64,
  #[getset(get = "pub")]
  pub subject: Option<String>,
  #[getset(get = "pub")]
  pub timestamp: String,
}
