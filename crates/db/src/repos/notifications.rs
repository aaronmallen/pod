//! Repository for character notification persistence.

use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};

use crate::{
  Error,
  entities::character_notification::{
    ActiveModel as NotifActive, Column as NotifColumn, Entity as NotifEntity, Model as NotifModel,
  },
};

/// Repository for character notification CRUD operations.
pub struct Repo<'a> {
  db: &'a DatabaseConnection,
}

impl<'a> Repo<'a> {
  /// Creates a new `Repo` bound to the given database connection.
  pub fn new(db: &'a DatabaseConnection) -> Self {
    Self {
      db,
    }
  }

  /// Returns all notification rows for the given character.
  pub async fn find_for_character(&self, character_id: i64) -> Result<Vec<NotifModel>, Error> {
    let rows = NotifEntity::find()
      .filter(NotifColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Upserts all notification rows for the given character using ON CONFLICT DO UPDATE.
  pub async fn upsert_for_character(&self, character_id: i64, notifications: &[NotifModel]) -> Result<(), Error> {
    for notif in notifications {
      let active = NotifActive {
        character_id: ActiveValue::Set(character_id),
        id: ActiveValue::NotSet,
        is_read: ActiveValue::Set(notif.is_read),
        notification_id: ActiveValue::Set(notif.notification_id),
        notif_type: ActiveValue::Set(notif.notif_type.clone()),
        sender_id: ActiveValue::Set(notif.sender_id),
        sender_type: ActiveValue::Set(notif.sender_type.clone()),
        synced_at: ActiveValue::Set(notif.synced_at.clone()),
        text: ActiveValue::Set(notif.text.clone()),
        timestamp: ActiveValue::Set(notif.timestamp.clone()),
      };
      NotifEntity::insert(active)
        .on_conflict(
          OnConflict::columns([NotifColumn::CharacterId, NotifColumn::NotificationId])
            .update_columns([
              NotifColumn::IsRead,
              NotifColumn::NotifType,
              NotifColumn::SenderId,
              NotifColumn::SenderType,
              NotifColumn::SyncedAt,
              NotifColumn::Text,
              NotifColumn::Timestamp,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }
}
