//! Repository for global tag and entity-tag assignment persistence.

use sea_orm::{
  ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait, sea_query::OnConflict,
};

use crate::{
  Error,
  entities::{
    entity_tag::{ActiveModel as EntityTagActive, Column as EntityTagColumn, Entity as EntityTagEntity},
    tag::{ActiveModel as TagActive, Column as TagColumn, Entity as TagEntity, Model as Tag},
  },
};

/// Repository for tag CRUD and entity-tag assignment operations.
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

  /// Returns all global tags.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn find_all(&self) -> Result<Vec<Tag>, Error> {
    Ok(TagEntity::find().all(self.db).await?)
  }

  /// Returns an existing tag by name, or inserts it and returns the new row.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn find_or_create(&self, name: &str) -> Result<Tag, Error> {
    if let Some(existing) = TagEntity::find().filter(TagColumn::Name.eq(name)).one(self.db).await? {
      return Ok(existing);
    }
    let active = TagActive {
      id: ActiveValue::NotSet,
      name: ActiveValue::Set(name.to_string()),
    };
    TagEntity::insert(active)
      .on_conflict(OnConflict::column(TagColumn::Name).do_nothing().to_owned())
      .exec(self.db)
      .await?;
    Ok(
      TagEntity::find()
        .filter(TagColumn::Name.eq(name))
        .one(self.db)
        .await?
        .expect("tag must exist after insert"),
    )
  }

  /// Replaces all tag assignments for an entity atomically.
  #[tracing::instrument(level = "trace", skip(self), fields(entity_id = entity_id))]
  pub async fn set_entity_tags(&self, entity_id: i64, entity_type: &str, tag_ids: Vec<i32>) -> Result<(), Error> {
    let entity_type = entity_type.to_string();
    self
      .db
      .transaction::<_, (), sea_orm::DbErr>(|txn| {
        Box::pin(async move {
          EntityTagEntity::delete_many()
            .filter(EntityTagColumn::EntityId.eq(entity_id))
            .filter(EntityTagColumn::EntityType.eq(&entity_type))
            .exec(txn)
            .await?;

          for tag_id in tag_ids {
            EntityTagEntity::insert(EntityTagActive {
              entity_id: ActiveValue::Set(entity_id),
              entity_type: ActiveValue::Set(entity_type.clone()),
              tag_id: ActiveValue::Set(tag_id),
            })
            .exec(txn)
            .await?;
          }

          Ok(())
        })
      })
      .await
      .map_err(|e| match e {
        sea_orm::TransactionError::Transaction(db_err) => Error::Database(db_err),
        sea_orm::TransactionError::Connection(db_err) => Error::Database(db_err),
      })
  }

  /// Returns all tags assigned to the given entity.
  #[tracing::instrument(level = "trace", skip(self), fields(entity_id = entity_id))]
  pub async fn tags_for_entity(&self, entity_id: i64, entity_type: &str) -> Result<Vec<Tag>, Error> {
    let tag_ids: Vec<i32> = EntityTagEntity::find()
      .filter(EntityTagColumn::EntityId.eq(entity_id))
      .filter(EntityTagColumn::EntityType.eq(entity_type))
      .all(self.db)
      .await?
      .into_iter()
      .map(|et| et.tag_id)
      .collect();

    if tag_ids.is_empty() {
      return Ok(Vec::new());
    }

    Ok(
      TagEntity::find()
        .filter(TagColumn::Id.is_in(tag_ids))
        .all(self.db)
        .await?,
    )
  }

  /// Replaces all tag assignments for a character atomically.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn set_character_tags(&self, character_id: i64, tag_ids: Vec<i32>) -> Result<(), Error> {
    self.set_entity_tags(character_id, "character", tag_ids).await
  }

  /// Returns all tags assigned to the given character.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn tags_for_character(&self, character_id: i64) -> Result<Vec<Tag>, Error> {
    self.tags_for_entity(character_id, "character").await
  }

  /// Replaces all tag assignments for a corporation atomically.
  #[tracing::instrument(level = "trace", skip(self), fields(corporation_id = corporation_id))]
  pub async fn set_corporation_tags(&self, corporation_id: i64, tag_ids: Vec<i32>) -> Result<(), Error> {
    self.set_entity_tags(corporation_id, "corporation", tag_ids).await
  }

  /// Returns all tags assigned to the given corporation.
  #[tracing::instrument(level = "trace", skip(self), fields(corporation_id = corporation_id))]
  pub async fn tags_for_corporation(&self, corporation_id: i64) -> Result<Vec<Tag>, Error> {
    self.tags_for_entity(corporation_id, "corporation").await
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

  mod find_all {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_tags_exist() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.find_all().await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_all_tags_after_creation() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.find_or_create("pvp").await.unwrap();
      repo.find_or_create("industry").await.unwrap();

      let result = repo.find_all().await.unwrap();
      assert_eq!(result.len(), 2);
    }
  }

  mod find_or_create {
    use super::*;

    #[tokio::test]
    async fn creates_new_tag_when_not_found() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let tag = repo.find_or_create("pvp").await.unwrap();
      assert_eq!(tag.name, "pvp");
    }

    #[tokio::test]
    async fn returns_existing_tag_when_found() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let first = repo.find_or_create("pvp").await.unwrap();
      let second = repo.find_or_create("pvp").await.unwrap();
      assert_eq!(first.id, second.id);
    }

    #[tokio::test]
    async fn creates_distinct_tags_for_different_names() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let a = repo.find_or_create("pvp").await.unwrap();
      let b = repo.find_or_create("pve").await.unwrap();
      assert_ne!(a.id, b.id);
    }
  }

  mod tags_for_entity {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_entity_has_no_tags() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.tags_for_entity(1, "character").await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_tags_after_set_entity_tags() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let tag = repo.find_or_create("pvp").await.unwrap();
      repo.set_entity_tags(1, "character", vec![tag.id]).await.unwrap();

      let result = repo.tags_for_entity(1, "character").await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].name, "pvp");
    }
  }

  mod set_entity_tags {
    use super::*;

    #[tokio::test]
    async fn replaces_previous_tags_atomically() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let pvp = repo.find_or_create("pvp").await.unwrap();
      let pve = repo.find_or_create("pve").await.unwrap();

      repo.set_entity_tags(1, "character", vec![pvp.id]).await.unwrap();
      repo.set_entity_tags(1, "character", vec![pve.id]).await.unwrap();

      let result = repo.tags_for_entity(1, "character").await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].name, "pve");
    }

    #[tokio::test]
    async fn set_empty_removes_all_tags() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let pvp = repo.find_or_create("pvp").await.unwrap();
      repo.set_entity_tags(1, "character", vec![pvp.id]).await.unwrap();
      repo.set_entity_tags(1, "character", vec![]).await.unwrap();

      let result = repo.tags_for_entity(1, "character").await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn does_not_affect_tags_for_different_entity_type() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let pvp = repo.find_or_create("pvp").await.unwrap();
      repo.set_entity_tags(1, "character", vec![pvp.id]).await.unwrap();
      repo.set_entity_tags(1, "corporation", vec![]).await.unwrap();

      let char_tags = repo.tags_for_entity(1, "character").await.unwrap();
      assert_eq!(char_tags.len(), 1);
    }
  }

  mod set_character_tags {
    use super::*;

    #[tokio::test]
    async fn delegates_to_set_entity_tags_with_character_type() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);

      let pvp = repo.find_or_create("pvp").await.unwrap();
      repo.set_character_tags(1, vec![pvp.id]).await.unwrap();

      let result = repo.tags_for_character(1).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].name, "pvp");
    }
  }
}
