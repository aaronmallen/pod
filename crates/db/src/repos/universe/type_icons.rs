//! Repository for cached EVE type icon persistence.

use sea_orm::{ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};

use crate::{
  Error,
  entities::type_icon::{ActiveModel, Column, Entity},
};

/// Repository for type icon cache CRUD operations.
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

  /// Returns all cached icon bytes as `(type_id, variant, data)` tuples.
  pub async fn find_all(&self) -> Result<Vec<(i32, String, Vec<u8>)>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(|r| (r.type_id, r.variant, r.data)).collect())
  }

  /// Returns cached icon bytes for the given type IDs and variant,
  /// as `(type_id, data)` tuples.
  pub async fn find_by_ids(&self, ids: &[i32], variant: &str) -> Result<Vec<(i32, Vec<u8>)>, Error> {
    let rows = Entity::find()
      .filter(Column::TypeId.is_in(ids.to_vec()))
      .filter(Column::Variant.eq(variant))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(|r| (r.type_id, r.data)).collect())
  }

  /// Inserts or replaces icon bytes for a `(type_id, variant)` pair.
  pub async fn upsert(&self, type_id: i32, variant: &str, data: Vec<u8>) -> Result<(), Error> {
    let model = ActiveModel {
      type_id: Set(type_id),
      variant: Set(variant.to_string()),
      data: Set(data),
    };
    Entity::insert(model)
      .on_conflict(
        OnConflict::columns([Column::TypeId, Column::Variant])
          .update_column(Column::Data)
          .to_owned(),
      )
      .exec(self.db)
      .await?;
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

  mod find_all {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_icons() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      assert!(repo.find_all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn returns_all_after_upsert() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(34, "64", vec![1, 2, 3]).await.unwrap();
      repo.upsert(35, "64", vec![4, 5, 6]).await.unwrap();
      let result = repo.find_all().await.unwrap();
      assert_eq!(result.len(), 2);
    }
  }

  mod find_by_ids {
    use super::*;

    #[tokio::test]
    async fn returns_empty_for_empty_ids() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      assert!(repo.find_by_ids(&[], "64").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn returns_matching_icons_for_variant() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(34, "64", vec![1, 2, 3]).await.unwrap();
      repo.upsert(34, "32", vec![7, 8, 9]).await.unwrap();
      repo.upsert(35, "64", vec![4, 5, 6]).await.unwrap();

      let result = repo.find_by_ids(&[34], "64").await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].0, 34);
      assert_eq!(result[0].1, vec![1, 2, 3]);
    }
  }

  mod upsert {
    use super::*;

    #[tokio::test]
    async fn inserts_icon_data() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(34, "64", vec![1, 2, 3]).await.unwrap();
      let result = repo.find_all().await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].2, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn updates_existing_icon_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(34, "64", vec![1, 2, 3]).await.unwrap();
      repo.upsert(34, "64", vec![9, 8, 7]).await.unwrap();
      let result = repo.find_all().await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].2, vec![9, 8, 7]);
    }
  }
}
