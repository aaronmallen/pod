//! Repository for global tag and entity-tag assignment persistence.

use sea_orm::{
  ActiveValue, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,
  TransactionTrait,
  sea_query::{Expr, OnConflict},
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

  /// Creates a new tag with the given name, appended at the end of the sort order.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn create(&self, name: &str) -> Result<Tag, Error> {
    let next_sort_order = TagEntity::find()
      .all(self.db)
      .await?
      .into_iter()
      .map(|t| t.sort_order)
      .max()
      .map(|m| m + 1)
      .unwrap_or(0);
    let active = TagActive {
      color: ActiveValue::NotSet,
      id: ActiveValue::NotSet,
      name: ActiveValue::Set(name.to_string()),
      sort_order: ActiveValue::Set(next_sort_order),
    };
    let result = TagEntity::insert(active).exec(self.db).await?;
    Ok(
      TagEntity::find_by_id(result.last_insert_id)
        .one(self.db)
        .await?
        .expect("tag must exist after insert"),
    )
  }

  /// Deletes a tag and its entity-tag assignments.
  #[tracing::instrument(level = "trace", skip(self), fields(id = id))]
  pub async fn delete(&self, id: i32) -> Result<(), Error> {
    EntityTagEntity::delete_many()
      .filter(EntityTagColumn::TagId.eq(id))
      .exec(self.db)
      .await?;
    TagEntity::delete_by_id(id).exec(self.db).await?;
    Ok(())
  }

  /// Returns all global tags ordered by sort_order ascending.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn find_all(&self) -> Result<Vec<Tag>, Error> {
    Ok(
      TagEntity::find()
        .order_by_asc(TagColumn::SortOrder)
        .all(self.db)
        .await?,
    )
  }

  /// Returns an existing tag by name, or inserts it and returns the new row.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn find_or_create(&self, name: &str) -> Result<Tag, Error> {
    if let Some(existing) = TagEntity::find().filter(TagColumn::Name.eq(name)).one(self.db).await? {
      return Ok(existing);
    }
    let active = TagActive {
      color: ActiveValue::NotSet,
      id: ActiveValue::NotSet,
      name: ActiveValue::Set(name.to_string()),
      sort_order: ActiveValue::NotSet,
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

  /// Renames a tag, returning the updated row.
  #[tracing::instrument(level = "trace", skip(self), fields(id = id))]
  pub async fn rename(&self, id: i32, name: &str) -> Result<Tag, Error> {
    TagEntity::update_many()
      .col_expr(TagColumn::Name, Expr::value(name.to_string()))
      .filter(TagColumn::Id.eq(id))
      .exec(self.db)
      .await?;
    Ok(
      TagEntity::find_by_id(id)
        .one(self.db)
        .await?
        .expect("tag must exist after rename"),
    )
  }

  /// Updates the sort order for a slice of tag IDs, assigning each its slice index.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn reorder(&self, ordered_ids: &[i32]) -> Result<(), Error> {
    let ordered_ids = ordered_ids.to_vec();
    self
      .db
      .transaction::<_, (), sea_orm::DbErr>(|txn| {
        Box::pin(async move {
          for (i, id) in ordered_ids.iter().enumerate() {
            TagEntity::update_many()
              .col_expr(TagColumn::SortOrder, Expr::value(i as i32))
              .filter(TagColumn::Id.eq(*id))
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

  /// Replaces all tag assignments for a character atomically.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn set_character_tags(&self, character_id: i64, tag_ids: Vec<i32>) -> Result<(), Error> {
    self.set_entity_tags(character_id, "character", tag_ids).await
  }

  /// Sets or clears the color for a tag, returning the updated row.
  #[tracing::instrument(level = "trace", skip(self), fields(id = id))]
  pub async fn set_color(&self, id: i32, color: Option<&str>) -> Result<Tag, Error> {
    TagEntity::update_many()
      .col_expr(TagColumn::Color, Expr::value(color.map(|s| s.to_string())))
      .filter(TagColumn::Id.eq(id))
      .exec(self.db)
      .await?;
    Ok(
      TagEntity::find_by_id(id)
        .one(self.db)
        .await?
        .expect("tag must exist after set_color"),
    )
  }

  /// Replaces all tag assignments for a corporation atomically.
  #[tracing::instrument(level = "trace", skip(self), fields(corporation_id = corporation_id))]
  pub async fn set_corporation_tags(&self, corporation_id: i64, tag_ids: Vec<i32>) -> Result<(), Error> {
    self.set_entity_tags(corporation_id, "corporation", tag_ids).await
  }

  /// Replaces all tag assignments for an entity atomically.
  #[tracing::instrument(level = "trace", skip(self), fields(entity_id = entity_id))]
  pub async fn set_entity_tags(&self, entity_id: i64, entity_type: &str, tag_ids: Vec<i32>) -> Result<(), Error> {
    let entity_type = entity_type.to_string();
    self
      .db
      .transaction::<_, (), sea_orm::DbErr>(|txn| Box::pin(replace_entity_tags(txn, entity_id, entity_type, tag_ids)))
      .await
      .map_err(map_txn_err)
  }

  /// Returns all tags assigned to the given character.
  #[tracing::instrument(level = "trace", skip(self), fields(character_id = character_id))]
  pub async fn tags_for_character(&self, character_id: i64) -> Result<Vec<Tag>, Error> {
    self.tags_for_entity(character_id, "character").await
  }

  /// Returns all tags assigned to the given corporation.
  #[tracing::instrument(level = "trace", skip(self), fields(corporation_id = corporation_id))]
  pub async fn tags_for_corporation(&self, corporation_id: i64) -> Result<Vec<Tag>, Error> {
    self.tags_for_entity(corporation_id, "corporation").await
  }

  /// Returns all tags assigned to the given entity, ordered by sort_order.
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
        .order_by_asc(TagColumn::SortOrder)
        .all(self.db)
        .await?,
    )
  }
}

async fn replace_entity_tags(
  txn: &DatabaseTransaction,
  entity_id: i64,
  entity_type: String,
  tag_ids: Vec<i32>,
) -> Result<(), sea_orm::DbErr> {
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
}

fn map_txn_err(e: sea_orm::TransactionError<sea_orm::DbErr>) -> Error {
  match e {
    sea_orm::TransactionError::Transaction(db_err) => Error::Database(db_err),
    sea_orm::TransactionError::Connection(db_err) => Error::Database(db_err),
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

  mod create {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_a_tag_with_the_given_name() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let tag = repo.create("pvp").await.unwrap();

      assert_eq!(tag.name, "pvp");
    }

    #[tokio::test]
    async fn it_sets_color_to_none() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let tag = repo.create("pvp").await.unwrap();

      assert_eq!(tag.color, None);
    }

    #[tokio::test]
    async fn it_assigns_ascending_sort_orders() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let first = repo.create("alpha").await.unwrap();
      let second = repo.create("beta").await.unwrap();

      assert!(first.sort_order < second.sort_order);
    }

    #[tokio::test]
    async fn it_starts_at_sort_order_zero_when_table_is_empty() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let tag = repo.create("pvp").await.unwrap();

      assert_eq!(tag.sort_order, 0);
    }
  }

  mod delete {
    use super::*;

    #[tokio::test]
    async fn it_removes_the_tag() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let tag = repo.create("pvp").await.unwrap();
      repo.delete(tag.id).await.unwrap();

      let all = repo.find_all().await.unwrap();
      assert!(all.is_empty());
    }

    #[tokio::test]
    async fn it_removes_entity_tag_assignments() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);
      let tag = repo.create("pvp").await.unwrap();
      repo.set_character_tags(1, vec![tag.id]).await.unwrap();
      repo.delete(tag.id).await.unwrap();

      let result = repo.tags_for_character(1).await.unwrap();
      assert!(result.is_empty());
    }
  }

  mod find_all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_tags_exist() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.find_all().await.unwrap();

      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn it_returns_all_tags_ordered_by_sort_order() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.create("gamma").await.unwrap();
      repo.create("alpha").await.unwrap();
      repo.reorder(&[2, 1]).await.unwrap();

      let result = repo.find_all().await.unwrap();
      assert_eq!(result[0].sort_order, 0);
      assert_eq!(result[1].sort_order, 1);
    }
  }

  mod find_or_create {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_new_tag_when_not_found() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let tag = repo.find_or_create("pvp").await.unwrap();

      assert_eq!(tag.name, "pvp");
    }

    #[tokio::test]
    async fn it_returns_existing_tag_when_found() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let first = repo.find_or_create("pvp").await.unwrap();
      let second = repo.find_or_create("pvp").await.unwrap();

      assert_eq!(first.id, second.id);
    }

    #[tokio::test]
    async fn it_creates_distinct_tags_for_different_names() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let a = repo.find_or_create("pvp").await.unwrap();
      let b = repo.find_or_create("pve").await.unwrap();

      assert_ne!(a.id, b.id);
    }
  }

  mod rename {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_updates_the_tag_name() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let tag = repo.create("pvp").await.unwrap();
      let updated = repo.rename(tag.id, "pve").await.unwrap();

      assert_eq!(updated.name, "pve");
    }

    #[tokio::test]
    async fn it_preserves_other_fields() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let tag = repo.create("pvp").await.unwrap();
      let updated = repo.rename(tag.id, "pve").await.unwrap();

      assert_eq!(updated.id, tag.id);
      assert_eq!(updated.sort_order, tag.sort_order);
    }
  }

  mod reorder {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_assigns_sort_order_by_slice_index() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let a = repo.create("alpha").await.unwrap();
      let b = repo.create("beta").await.unwrap();
      repo.reorder(&[b.id, a.id]).await.unwrap();

      let result = repo.find_all().await.unwrap();
      assert_eq!(result[0].name, "beta");
      assert_eq!(result[1].name, "alpha");
    }
  }

  mod set_character_tags {
    use super::*;

    #[tokio::test]
    async fn it_delegates_to_set_entity_tags_with_character_type() {
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

  mod set_color {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_sets_the_color() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let tag = repo.create("pvp").await.unwrap();
      let updated = repo.set_color(tag.id, Some("#ff0000")).await.unwrap();

      assert_eq!(updated.color, Some("#ff0000".to_string()));
    }

    #[tokio::test]
    async fn it_clears_the_color_when_passed_none() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let tag = repo.create("pvp").await.unwrap();
      repo.set_color(tag.id, Some("#ff0000")).await.unwrap();
      let updated = repo.set_color(tag.id, None).await.unwrap();

      assert_eq!(updated.color, None);
    }
  }

  mod set_entity_tags {
    use super::*;

    #[tokio::test]
    async fn it_replaces_previous_tags_atomically() {
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
    async fn it_removes_all_tags_when_given_empty_list() {
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
    async fn it_does_not_affect_tags_for_different_entity_type() {
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

  mod tags_for_entity {
    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_entity_has_no_tags() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.tags_for_entity(1, "character").await.unwrap();

      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn it_returns_tags_ordered_by_sort_order() {
      let db = setup_db().await;
      insert_character(&db, 1, "Alice").await;
      let repo = Repo::new(&db);
      let a = repo.create("alpha").await.unwrap();
      let b = repo.create("beta").await.unwrap();
      repo.reorder(&[b.id, a.id]).await.unwrap();
      repo.set_entity_tags(1, "character", vec![a.id, b.id]).await.unwrap();

      let result = repo.tags_for_entity(1, "character").await.unwrap();
      assert_eq!(result[0].name, "beta");
      assert_eq!(result[1].name, "alpha");
    }
  }
}
