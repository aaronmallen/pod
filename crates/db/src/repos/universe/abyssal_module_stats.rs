//! Repository for abyssal module stat bound persistence.

use pod_model::AbyssalModuleStat;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};

use crate::{
  Error,
  entities::abyssal_module_stat::{ActiveModel, Column, Entity},
};

/// Repository for abyssal module stat CRUD operations.
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

  /// Returns all abyssal module stats for the given abyssal type ID.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn find_by_type_id(&self, abyssal_type_id: i32) -> Result<Vec<AbyssalModuleStat>, Error> {
    let rows = Entity::find()
      .filter(Column::AbyssalTypeId.eq(abyssal_type_id))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Returns all abyssal module stats for any of the given type IDs.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn find_by_type_ids(&self, type_ids: &[i32]) -> Result<Vec<AbyssalModuleStat>, Error> {
    if type_ids.is_empty() {
      return Ok(Vec::new());
    }
    let rows = Entity::find()
      .filter(Column::AbyssalTypeId.is_in(type_ids.to_vec()))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Bulk-upserts abyssal module stat rows in chunks of 500.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn upsert_many(&self, records: &[AbyssalModuleStat]) -> Result<(), Error> {
    if records.is_empty() {
      return Ok(());
    }
    let active: Vec<ActiveModel> = records.iter().cloned().map(Into::into).collect();
    for chunk in active.chunks(500) {
      Entity::insert_many(chunk.to_vec())
        .on_conflict(
          OnConflict::columns([Column::AbyssalTypeId, Column::AttributeId])
            .update_columns([Column::MaxMult, Column::MinMult])
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
  use pod_model::AbyssalModuleStat;
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  mod find_by_type_id {
    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_stats() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      assert!(repo.find_by_type_id(47408).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_returns_stats_for_type() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let records = vec![
        AbyssalModuleStat::new(47408, 6, 0.6, 1.4),
        AbyssalModuleStat::new(47408, 20, 0.9, 1.1),
      ];
      repo.upsert_many(&records).await.unwrap();

      let result = repo.find_by_type_id(47408).await.unwrap();

      assert_eq!(result.len(), 2);
    }
  }

  mod find_by_type_ids {
    use super::*;

    #[tokio::test]
    async fn it_returns_empty_when_no_ids() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      assert!(repo.find_by_type_ids(&[]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_returns_empty_when_no_matching_stats() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      assert!(repo.find_by_type_ids(&[99999]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_returns_stats_for_multiple_type_ids() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let records = vec![
        AbyssalModuleStat::new(47408, 6, 0.6, 1.4),
        AbyssalModuleStat::new(47409, 6, 0.7, 1.3),
        AbyssalModuleStat::new(47410, 6, 0.8, 1.2),
      ];
      repo.upsert_many(&records).await.unwrap();

      let result = repo.find_by_type_ids(&[47408, 47410]).await.unwrap();

      assert_eq!(result.len(), 2);
      let mut type_ids: Vec<i32> = result.iter().map(|r| *r.abyssal_type_id()).collect();
      type_ids.sort_unstable();
      assert_eq!(type_ids, vec![47408, 47410]);
    }

    #[tokio::test]
    async fn it_returns_all_stats_across_type_ids() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let records = vec![
        AbyssalModuleStat::new(47408, 6, 0.6, 1.4),
        AbyssalModuleStat::new(47408, 20, 0.9, 1.1),
        AbyssalModuleStat::new(47409, 6, 0.7, 1.3),
      ];
      repo.upsert_many(&records).await.unwrap();

      let result = repo.find_by_type_ids(&[47408, 47409]).await.unwrap();

      assert_eq!(result.len(), 3);
    }
  }

  mod upsert_many {
    use super::*;

    #[tokio::test]
    async fn it_does_nothing_when_empty() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.upsert_many(&[]).await.unwrap();

      assert!(repo.find_by_type_id(1).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn it_inserts_multiple_stats() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let records = vec![
        AbyssalModuleStat::new(47408, 6, 0.6, 1.4),
        AbyssalModuleStat::new(47408, 50, 0.8, 1.5),
      ];

      repo.upsert_many(&records).await.unwrap();

      let result = repo.find_by_type_id(47408).await.unwrap();
      assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn it_updates_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let records = vec![AbyssalModuleStat::new(47408, 6, 0.6, 1.4)];
      repo.upsert_many(&records).await.unwrap();
      let updated = vec![AbyssalModuleStat::new(47408, 6, 0.5, 1.5)];

      repo.upsert_many(&updated).await.unwrap();

      let result = repo.find_by_type_id(47408).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(*result[0].min_mult(), 0.5);
      assert_eq!(*result[0].max_mult(), 1.5);
    }
  }
}
