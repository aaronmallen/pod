//! Repository for character contact and contact label persistence.

use sea_orm::{ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};

use crate::{
  Error,
  entities::{
    character_contact::{
      ActiveModel as ContactActive, Column as ContactColumn, Entity as ContactEntity, Model as ContactModel,
    },
    character_contact_label::{
      ActiveModel as LabelActive, Column as LabelColumn, Entity as LabelEntity, Model as LabelModel,
    },
  },
};

/// Repository for character contact and label CRUD operations.
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

  /// Returns all contact rows for the given character.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn find_for_character(&self, character_id: i64) -> Result<Vec<ContactModel>, Error> {
    let rows = ContactEntity::find()
      .filter(ContactColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Returns all contact label rows for the given character.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn find_labels_for_character(&self, character_id: i64) -> Result<Vec<LabelModel>, Error> {
    let rows = LabelEntity::find()
      .filter(LabelColumn::CharacterId.eq(character_id))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Upserts contacts and labels for the given character using ON CONFLICT DO UPDATE.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn upsert_for_character(
    &self,
    character_id: i64,
    contacts: &[ContactModel],
    labels: &[LabelModel],
  ) -> Result<(), Error> {
    for contact in contacts {
      let active = ContactActive {
        character_id: ActiveValue::Set(character_id),
        contact_id: ActiveValue::Set(contact.contact_id),
        contact_name: ActiveValue::Set(contact.contact_name.clone()),
        contact_type: ActiveValue::Set(contact.contact_type.clone()),
        id: ActiveValue::NotSet,
        is_watchlist: ActiveValue::Set(contact.is_watchlist),
        label_ids: ActiveValue::Set(contact.label_ids.clone()),
        standing: ActiveValue::Set(contact.standing),
        synced_at: ActiveValue::Set(contact.synced_at.clone()),
      };
      ContactEntity::insert(active)
        .on_conflict(
          OnConflict::columns([ContactColumn::CharacterId, ContactColumn::ContactId])
            .update_columns([
              ContactColumn::ContactName,
              ContactColumn::ContactType,
              ContactColumn::IsWatchlist,
              ContactColumn::LabelIds,
              ContactColumn::Standing,
              ContactColumn::SyncedAt,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }

    for label in labels {
      let active = LabelActive {
        character_id: ActiveValue::Set(character_id),
        id: ActiveValue::NotSet,
        label_id: ActiveValue::Set(label.label_id),
        label_name: ActiveValue::Set(label.label_name.clone()),
      };
      LabelEntity::insert(active)
        .on_conflict(
          OnConflict::columns([LabelColumn::CharacterId, LabelColumn::LabelId])
            .update_column(LabelColumn::LabelName)
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

  fn make_contact(character_id: i64, contact_id: i32, name: &str) -> ContactModel {
    ContactModel {
      character_id,
      contact_id,
      contact_name: name.to_string(),
      contact_type: "character".to_string(),
      id: 0,
      is_watchlist: false,
      label_ids: "[]".to_string(),
      standing: 5.0,
      synced_at: "2025-01-01T00:00:00Z".to_string(),
    }
  }

  fn make_label(character_id: i64, label_id: i32, label_name: &str) -> LabelModel {
    LabelModel {
      character_id,
      id: 0,
      label_id,
      label_name: label_name.to_string(),
    }
  }

  mod find_for_character {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_contacts() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let result = repo.find_for_character(1).await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_contacts_after_upsert() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let contact = make_contact(1, 100, "Bob");
      repo.upsert_for_character(1, &[contact], &[]).await.unwrap();

      let result = repo.find_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].contact_id, 100);
      assert_eq!(result[0].contact_name, "Bob");
    }

    #[tokio::test]
    async fn does_not_return_contacts_for_other_characters() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      insert_character(&db, 2, "Bob").await;
      let repo = Repo::new(&db);

      let contact = make_contact(1, 100, "Bob");
      repo.upsert_for_character(1, &[contact], &[]).await.unwrap();

      let result = repo.find_for_character(2).await.unwrap();
      assert!(result.is_empty());
    }
  }

  mod find_labels_for_character {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_labels() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let result = repo.find_labels_for_character(1).await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_labels_after_upsert() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let label = make_label(1, 42, "Alliance Pilots");
      repo.upsert_for_character(1, &[], &[label]).await.unwrap();

      let result = repo.find_labels_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].label_id, 42);
      assert_eq!(result[0].label_name, "Alliance Pilots");
    }
  }

  mod upsert_for_character {
    use super::*;

    #[tokio::test]
    async fn upserts_contacts_and_labels_together() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let contact = make_contact(1, 200, "Alice");
      let label = make_label(1, 10, "Friendlies");
      repo.upsert_for_character(1, &[contact], &[label]).await.unwrap();

      let contacts = repo.find_for_character(1).await.unwrap();
      let labels = repo.find_labels_for_character(1).await.unwrap();
      assert_eq!(contacts.len(), 1);
      assert_eq!(labels.len(), 1);
    }

    #[tokio::test]
    async fn updates_existing_contact_on_conflict() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let mut contact = make_contact(1, 300, "Charlie");
      contact.standing = 0.0;
      repo.upsert_for_character(1, &[contact], &[]).await.unwrap();

      let mut updated = make_contact(1, 300, "Charlie Updated");
      updated.standing = 10.0;
      repo.upsert_for_character(1, &[updated], &[]).await.unwrap();

      let result = repo.find_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].contact_name, "Charlie Updated");
      assert_eq!(result[0].standing, 10.0);
    }

    #[tokio::test]
    async fn updates_existing_label_on_conflict() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let label = make_label(1, 99, "Old Name");
      repo.upsert_for_character(1, &[], &[label]).await.unwrap();

      let updated = make_label(1, 99, "New Name");
      repo.upsert_for_character(1, &[], &[updated]).await.unwrap();

      let result = repo.find_labels_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].label_name, "New Name");
    }
  }
}
