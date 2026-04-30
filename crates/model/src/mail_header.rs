//! Domain model for mail headers.

use validator::Validate;

#[derive(Clone, Debug, Validate)]
pub struct Model {
  pub character_id: i64,
  pub mail_id: i64,
  pub subject: String,
  pub from_id: Option<i64>,
  pub is_read: bool,
  #[validate(length(min = 1))]
  pub timestamp: String,
  pub recipients_display: String,
}
