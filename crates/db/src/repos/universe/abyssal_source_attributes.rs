//! Repository for abyssal source-attribute mapping persistence.

use std::collections::{HashMap, HashSet};

use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, sea_query::OnConflict};

use crate::{
  Error,
  entities::abyssal_source_attribute::{ActiveModel, Column, Entity},
};

/// Repository for abyssal source-attribute mapping CRUD operations.
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

  /// Returns a map of `source_type_id -> Set<attr_id>` for the given
  /// source type IDs.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn find_attr_ids_by_source_type_ids(&self, ids: &[i32]) -> Result<HashMap<i32, HashSet<i32>>, Error> {
    if ids.is_empty() {
      return Ok(HashMap::new());
    }
    let rows = Entity::find()
      .filter(Column::SourceTypeId.is_in(ids.to_vec()))
      .all(self.db)
      .await?;
    let mut result: HashMap<i32, HashSet<i32>> = HashMap::new();
    for row in rows {
      result.entry(row.source_type_id).or_default().insert(row.attr_id);
    }
    Ok(result)
  }

  /// Bulk-upserts `(source_type_id, attr_id)` pairs in chunks of 500.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn upsert_many(&self, records: &[(i32, i32)]) -> Result<(), Error> {
    if records.is_empty() {
      return Ok(());
    }
    let active: Vec<ActiveModel> = records.iter().copied().map(Into::into).collect();
    for chunk in active.chunks(500) {
      match Entity::insert_many(chunk.to_vec())
        .on_conflict(
          OnConflict::columns([Column::SourceTypeId, Column::AttrId])
            .do_nothing()
            .to_owned(),
        )
        .exec(self.db)
        .await
      {
        Ok(_) | Err(DbErr::RecordNotInserted) => {}
        Err(e) => return Err(Error::Database(e)),
      }
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use std::collections::{HashMap, HashSet};

  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  mod find_attr_ids_by_source_type_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_ids_is_empty() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let result = repo.find_attr_ids_by_source_type_ids(&[]).await.unwrap();

      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn it_returns_empty_when_no_matching_rows() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let result = repo.find_attr_ids_by_source_type_ids(&[99999]).await.unwrap();

      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn it_returns_attr_ids_grouped_by_source_type() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert_many(&[(100, 6), (100, 20), (200, 6)]).await.unwrap();

      let result = repo.find_attr_ids_by_source_type_ids(&[100, 200]).await.unwrap();

      let expected: HashMap<i32, HashSet<i32>> = [(100, [6, 20].into()), (200, [6].into())].into_iter().collect();
      assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn it_filters_to_requested_source_type_ids() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert_many(&[(100, 6), (200, 20), (300, 50)]).await.unwrap();

      let result = repo.find_attr_ids_by_source_type_ids(&[100, 300]).await.unwrap();

      assert_eq!(result.len(), 2);
      assert!(result.contains_key(&100));
      assert!(result.contains_key(&300));
      assert!(!result.contains_key(&200));
    }
  }

  mod upsert_many {
    use super::*;

    #[tokio::test]
    async fn it_does_nothing_when_empty() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_many(&[]).await.unwrap();

      assert!(repo.find_attr_ids_by_source_type_ids(&[1]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_inserts_records() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_many(&[(100, 6), (100, 20)]).await.unwrap();

      let result = repo.find_attr_ids_by_source_type_ids(&[100]).await.unwrap();
      assert_eq!(result[&100].len(), 2);
    }

    #[tokio::test]
    async fn it_ignores_duplicate_rows() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert_many(&[(100, 6)]).await.unwrap();

      repo.upsert_many(&[(100, 6)]).await.unwrap();

      let result = repo.find_attr_ids_by_source_type_ids(&[100]).await.unwrap();
      assert_eq!(result[&100].len(), 1);
    }
  }
}
