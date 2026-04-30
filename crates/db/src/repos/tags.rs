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
  pub async fn find_all(&self) -> Result<Vec<Tag>, Error> {
    Ok(TagEntity::find().all(self.db).await?)
  }

  /// Returns an existing tag by name, or inserts it and returns the new row.
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
  pub async fn set_character_tags(&self, character_id: i64, tag_ids: Vec<i32>) -> Result<(), Error> {
    self.set_entity_tags(character_id, "character", tag_ids).await
  }

  /// Returns all tags assigned to the given character.
  pub async fn tags_for_character(&self, character_id: i64) -> Result<Vec<Tag>, Error> {
    self.tags_for_entity(character_id, "character").await
  }

  /// Replaces all tag assignments for a corporation atomically.
  pub async fn set_corporation_tags(&self, corporation_id: i64, tag_ids: Vec<i32>) -> Result<(), Error> {
    self.set_entity_tags(corporation_id, "corporation", tag_ids).await
  }

  /// Returns all tags assigned to the given corporation.
  pub async fn tags_for_corporation(&self, corporation_id: i64) -> Result<Vec<Tag>, Error> {
    self.tags_for_entity(corporation_id, "corporation").await
  }
}
