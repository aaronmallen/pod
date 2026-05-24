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
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn find_for_character(&self, character_id: i64) -> Result<Vec<NotifModel>, Error> {
    let rows = NotifEntity::find()
      .filter(NotifColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Upserts all notification rows for the given character using ON CONFLICT DO UPDATE.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
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

#[cfg(test)]
mod tests {
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  async fn insert_character(db: &DatabaseConnection, id: i64, name: &str) {
    use sea_orm::ActiveValue::Set;
    crate::entities::character::Entity::insert(crate::entities::character::ActiveModel {
      access_token: Set(String::new()),
      charisma: Set(None),
      corp_id: Set(0),
      corp_name: Set(String::new()),
      granted_scopes: Set(None),
      id: Set(id),
      intelligence: Set(None),
      isk_balance: Set(None),
      location_docked: Set(None),
      location_name: Set(None),
      memory: Set(None),
      name: Set(name.to_string()),
      perception: Set(None),
      portrait_tone: Set(0),
      refresh_token: Set(String::new()),
      sort_order: Set(0),
      token_expires_at: Set(0),
      willpower: Set(None),
    })
    .exec(db)
    .await
    .unwrap();
  }

  fn make_notification(character_id: i64, notification_id: i32) -> NotifModel {
    NotifModel {
      character_id,
      id: 0,
      is_read: false,
      notification_id,
      notif_type: "StructureUnderAttack".to_string(),
      sender_id: Some(12345),
      sender_type: Some("character".to_string()),
      synced_at: "2025-01-01T00:00:00Z".to_string(),
      text: Some("Your structure is under attack!".to_string()),
      timestamp: "2025-01-01T00:00:00Z".to_string(),
    }
  }

  mod find_for_character {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_notifications() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let result = repo.find_for_character(1).await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_notifications_after_upsert() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let notif = make_notification(1, 1001);
      repo.upsert_for_character(1, &[notif]).await.unwrap();

      let result = repo.find_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].notification_id, 1001);
    }

    #[tokio::test]
    async fn does_not_return_notifications_for_other_characters() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      insert_character(&db, 2, "Bob").await;
      let repo = Repo::new(&db);

      let notif = make_notification(1, 1001);
      repo.upsert_for_character(1, &[notif]).await.unwrap();

      let result = repo.find_for_character(2).await.unwrap();
      assert!(result.is_empty());
    }
  }

  mod upsert_for_character {
    use super::*;

    #[tokio::test]
    async fn upserts_multiple_notifications() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let n1 = make_notification(1, 1001);
      let n2 = make_notification(1, 1002);
      repo.upsert_for_character(1, &[n1, n2]).await.unwrap();

      let result = repo.find_for_character(1).await.unwrap();
      assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn updates_existing_notification_on_conflict() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let mut notif = make_notification(1, 1001);
      notif.is_read = false;
      repo.upsert_for_character(1, &[notif]).await.unwrap();

      let mut updated = make_notification(1, 1001);
      updated.is_read = true;
      repo.upsert_for_character(1, &[updated]).await.unwrap();

      let result = repo.find_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert!(result[0].is_read);
    }
  }
}
