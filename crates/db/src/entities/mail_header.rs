//! Database entity for mail headers.

use pod_model::MailHeader;
use sea_orm::prelude::*;
use validator::Validate;

/// A mail header stored in the `mail_headers` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq, Validate)]
#[sea_orm(table_name = "mail_headers")]
pub struct Model {
  /// Cached plain-text mail body, or `None` if not yet fetched.
  pub body: Option<String>,
  /// ID of the owning character.
  pub character_id: i64,
  /// ESI ID of the sender.
  pub from_id: Option<i64>,
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i32,
  /// Whether the character has read this mail.
  pub is_read: bool,
  /// ESI mail ID.
  pub mail_id: i64,
  /// Short body preview (~250 chars), or `None` if not yet fetched.
  pub preview: Option<String>,
  /// Pre-formatted comma-separated recipient display names.
  pub recipients_display: String,
  /// Mail subject line.
  pub subject: String,
  /// ISO 8601 send timestamp.
  pub timestamp: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for MailHeader {
  fn from(e: Model) -> Self {
    Self {
      body: e.body,
      character_id: e.character_id,
      from_id: e.from_id,
      is_read: e.is_read,
      mail_id: e.mail_id,
      preview: e.preview,
      recipients_display: e.recipients_display,
      subject: e.subject,
      timestamp: e.timestamp,
    }
  }
}
