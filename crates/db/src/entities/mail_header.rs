//! Database entity for mail headers.

use pod_model::MailHeader;
use sea_orm::prelude::*;
use validator::Validate;

/// A mail header stored in the `mail_headers` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq, Validate)]
#[sea_orm(table_name = "mail_headers")]
pub struct Model {
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i32,
  /// ID of the owning character.
  pub character_id: i64,
  /// ESI mail ID.
  pub mail_id: i64,
  /// Mail subject line.
  pub subject: String,
  /// ESI ID of the sender.
  pub from_id: Option<i64>,
  /// Whether the character has read this mail.
  pub is_read: bool,
  /// ISO 8601 send timestamp.
  pub timestamp: String,
  /// Pre-formatted comma-separated recipient display names.
  pub recipients_display: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for MailHeader {
  fn from(e: Model) -> Self {
    Self {
      character_id: e.character_id,
      mail_id: e.mail_id,
      subject: e.subject,
      from_id: e.from_id,
      is_read: e.is_read,
      timestamp: e.timestamp,
      recipients_display: e.recipients_display,
    }
  }
}
