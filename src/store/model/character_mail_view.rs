use sqlx::FromRow;

use crate::store::model::{CharacterMail, CharacterMailBody, CharacterMailRecipient};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailRender {
  pub body: CharacterMailBody,
  pub header: CharacterMail,
  pub label_ids: Vec<i64>,
  pub recipients: Vec<CharacterMailRecipient>,
  pub recipients_display: String,
}

#[derive(Clone, Debug, Eq, FromRow, PartialEq)]
pub struct UnifiedMail {
  pub body: String,
  pub character_id: i64,
  pub from_corp: bool,
  pub from_id: i64,
  pub from_name: String,
  pub from_system: bool,
  pub has_attachment: bool,
  pub important: bool,
  pub is_read: bool,
  pub mail_id: i64,
  pub subject: Option<String>,
  pub timestamp: String,
}
