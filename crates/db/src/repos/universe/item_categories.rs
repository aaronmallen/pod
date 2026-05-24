//! Repository for item category persistence.

use pod_model::ItemCategory;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::item_category::{ActiveModel, Column, Entity},
};

/// Repository for item category CRUD operations.
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

  /// Returns all item categories.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn all(&self) -> Result<Vec<ItemCategory>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Finds an item category by its unique ID.
  #[tracing::instrument(level = "trace", skip(self), fields(id = id))]
  pub async fn find(&self, id: i32) -> Result<Option<ItemCategory>, Error> {
    let row = Entity::find_by_id(id).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Finds an item category by its display name (exact match).
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn find_by_name(&self, name: &str) -> Result<Option<ItemCategory>, Error> {
    let row = Entity::find().filter(Column::Name.eq(name)).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Returns raw entity rows for the given category IDs.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn find_by_ids(&self, ids: &[i32]) -> Result<Vec<crate::entities::item_category::Model>, Error> {
    let rows = Entity::find()
      .filter(Column::Id.is_in(ids.to_vec()))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Inserts or updates an item category row.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn upsert(&self, record: &ItemCategory) -> Result<(), Error> {
    record.validate()?;
    let active: ActiveModel = record.clone().into();
    Entity::insert(active)
      .on_conflict(
        OnConflict::column(Column::Id)
          .update_columns([Column::Name, Column::Published])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Bulk-upserts item category rows in chunks of 500.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn upsert_many(&self, records: &[ItemCategory]) -> Result<(), Error> {
    if records.is_empty() {
      return Ok(());
    }
    let mut active = Vec::with_capacity(records.len());
    for record in records {
      record.validate()?;
      active.push(ActiveModel::from(record.clone()));
    }
    for chunk in active.chunks(500) {
      Entity::insert_many(chunk.to_vec())
        .on_conflict(
          OnConflict::column(Column::Id)
            .update_columns([Column::Name, Column::Published])
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
  use pod_model::ItemCategory;
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  fn make_category(id: i32, name: &str) -> ItemCategory {
    ItemCategory::new(id, name)
  }

  mod all {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_categories() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      assert!(repo.all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn returns_all_after_upsert() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_category(1, "Ships")).await.unwrap();
      repo.upsert(&make_category(2, "Modules")).await.unwrap();
      assert_eq!(repo.all().await.unwrap().len(), 2);
    }
  }

  mod find {
    use super::*;

    #[tokio::test]
    async fn returns_none_when_not_found() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      assert!(repo.find(999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn returns_some_after_upsert() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_category(6, "Ships")).await.unwrap();
      let result = repo.find(6).await.unwrap();
      assert!(result.is_some());
      assert_eq!(*result.unwrap().id(), 6);
    }
  }

  mod find_by_name {
    use super::*;

    #[tokio::test]
    async fn returns_none_when_not_found() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      assert!(repo.find_by_name("Nonexistent").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn returns_category_by_name() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_category(6, "Ships")).await.unwrap();
      assert!(repo.find_by_name("Ships").await.unwrap().is_some());
    }
  }

  mod find_by_ids {
    use super::*;

    #[tokio::test]
    async fn returns_empty_for_empty_ids() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.find_by_ids(&[]).await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_matching_categories() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_category(6, "Ships")).await.unwrap();
      repo.upsert(&make_category(7, "Modules")).await.unwrap();
      let result = repo.find_by_ids(&[6]).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].id, 6);
    }
  }

  mod upsert_many {
    use super::*;

    #[tokio::test]
    async fn does_nothing_when_empty() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert_many(&[]).await.unwrap();
      assert!(repo.all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn inserts_multiple_categories() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let records = vec![make_category(1, "Ships"), make_category(2, "Modules")];
      repo.upsert_many(&records).await.unwrap();
      assert_eq!(repo.all().await.unwrap().len(), 2);
    }
  }
}
