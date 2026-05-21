//! Repository for race persistence.

use pod_model::Race;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::race::{ActiveModel, Column, Entity},
};

/// Repository for race CRUD operations.
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

  /// Returns all races.
  pub async fn all(&self) -> Result<Vec<Race>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Finds a race by its unique ID.
  pub async fn find(&self, id: i32) -> Result<Option<Race>, Error> {
    let row = Entity::find_by_id(id).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Finds a race by its display name (exact match).
  pub async fn find_by_name(&self, name: &str) -> Result<Option<Race>, Error> {
    let row = Entity::find().filter(Column::Name.eq(name)).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Inserts or updates a race row.
  pub async fn upsert(&self, record: &Race) -> Result<(), Error> {
    record.validate()?;
    let active: ActiveModel = record.clone().into();
    Entity::insert(active)
      .on_conflict(
        OnConflict::column(Column::Id)
          .update_columns([Column::AllianceId, Column::Description, Column::Name])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Bulk-upserts race rows in a single batch.
  pub async fn upsert_many(&self, records: &[Race]) -> Result<(), Error> {
    if records.is_empty() {
      return Ok(());
    }
    let mut active = Vec::with_capacity(records.len());
    for record in records {
      record.validate()?;
      active.push(ActiveModel::from(record.clone()));
    }
    Entity::insert_many(active)
      .on_conflict(
        OnConflict::column(Column::Id)
          .update_columns([Column::AllianceId, Column::Description, Column::Name])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use pod_model::Race;
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  fn make_race(id: i32, name: &str) -> Race {
    Race::new(id, name)
  }

  mod all {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_races() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      assert!(repo.all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn returns_all_after_upsert() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_race(1, "Caldari")).await.unwrap();
      repo.upsert(&make_race(2, "Gallente")).await.unwrap();
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
      repo.upsert(&make_race(1, "Caldari")).await.unwrap();
      let result = repo.find(1).await.unwrap();
      assert!(result.is_some());
      assert_eq!(*result.unwrap().id(), 1);
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
    async fn returns_race_by_name() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_race(1, "Caldari")).await.unwrap();
      assert!(repo.find_by_name("Caldari").await.unwrap().is_some());
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
    async fn inserts_multiple_races() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let records = vec![make_race(1, "Caldari"), make_race(2, "Gallente")];
      repo.upsert_many(&records).await.unwrap();
      assert_eq!(repo.all().await.unwrap().len(), 2);
    }
  }
}
