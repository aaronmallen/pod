//! Repository for abyssal source type ID persistence.

use sea_orm::{DatabaseConnection, EntityTrait};

use crate::{
  Error,
  entities::abyssal_source_type::{ActiveModel, Entity},
};

/// Repository for abyssal source type CRUD operations.
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

  /// Returns all source type IDs.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn all_source_type_ids(&self) -> Result<Vec<i32>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(|r| r.source_type_id).collect())
  }

  /// Replaces all source type IDs with the given set.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn replace_all(&self, ids: &[i32]) -> Result<(), Error> {
    if ids.is_empty() {
      return Ok(());
    }
    Entity::delete_many().exec(self.db).await?;
    let active: Vec<ActiveModel> = ids.iter().copied().map(Into::into).collect();
    for chunk in active.chunks(500) {
      Entity::insert_many(chunk.to_vec()).exec(self.db).await?;
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

  mod all_source_type_ids {
    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_records() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      assert!(repo.all_source_type_ids().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_returns_all_ids_after_replace() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.replace_all(&[10001, 10002, 10003]).await.unwrap();

      let mut result = repo.all_source_type_ids().await.unwrap();
      result.sort_unstable();

      assert_eq!(result, vec![10001, 10002, 10003]);
    }
  }

  mod replace_all {
    use super::*;

    #[tokio::test]
    async fn it_does_nothing_when_empty() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.replace_all(&[]).await.unwrap();

      assert!(repo.all_source_type_ids().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_inserts_ids() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.replace_all(&[10001, 10002]).await.unwrap();

      let mut result = repo.all_source_type_ids().await.unwrap();
      result.sort_unstable();
      assert_eq!(result, vec![10001, 10002]);
    }

    #[tokio::test]
    async fn it_replaces_existing_ids() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.replace_all(&[10001, 10002]).await.unwrap();

      repo.replace_all(&[20001, 20002, 20003]).await.unwrap();

      let mut result = repo.all_source_type_ids().await.unwrap();
      result.sort_unstable();
      assert_eq!(result, vec![20001, 20002, 20003]);
    }
  }
}
