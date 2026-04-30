//! Database entity for EVE Online character notifications.

use sea_orm::prelude::*;

/// A notification record stored in the `character_notifications` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "character_notifications")]
pub struct Model {
  /// ID of the owning character.
  pub character_id: i64,
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i32,
  /// Whether the character has read this notification.
  pub is_read: bool,
  /// Unique EVE notification identifier.
  pub notification_id: i32,
  /// Raw EVE notification type string.
  pub notif_type: String,
  /// Sender entity ID, if available.
  pub sender_id: Option<i32>,
  /// Sender entity type, if available.
  pub sender_type: Option<String>,
  /// ISO-8601 timestamp when this record was last synced from ESI.
  pub synced_at: String,
  /// Optional notification body text.
  pub text: Option<String>,
  /// ISO-8601 timestamp when the notification was sent.
  pub timestamp: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
