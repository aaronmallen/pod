//! Repository for star persistence.

use pod_model::Star;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::star::{ActiveModel, Column, Entity},
};

/// Repository for star CRUD operations.
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

  /// Returns all stars.
  pub async fn all(&self) -> Result<Vec<Star>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Finds a star by its unique ID.
  pub async fn find(&self, id: i32) -> Result<Option<Star>, Error> {
    let row = Entity::find_by_id(id).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Finds a star by its display name (exact match).
  pub async fn find_by_name(&self, name: &str) -> Result<Option<Star>, Error> {
    let row = Entity::find().filter(Column::Name.eq(name)).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Inserts or updates a star row.
  pub async fn upsert(&self, record: &Star) -> Result<(), Error> {
    record.validate()?;
    let active: ActiveModel = record.clone().into();
    Entity::insert(active)
      .on_conflict(
        OnConflict::column(Column::Id)
          .update_columns([
            Column::Age,
            Column::ItemTypeId,
            Column::Luminosity,
            Column::Name,
            Column::Radius,
            Column::SolarSystemId,
            Column::SpectralClass,
            Column::Temperature,
          ])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Bulk-upserts star rows in chunks of 200.
  pub async fn upsert_many(&self, records: &[Star]) -> Result<(), Error> {
    if records.is_empty() {
      return Ok(());
    }
    let mut active = Vec::with_capacity(records.len());
    for record in records {
      record.validate()?;
      active.push(ActiveModel::from(record.clone()));
    }
    for chunk in active.chunks(200) {
      Entity::insert_many(chunk.to_vec())
        .on_conflict(
          OnConflict::column(Column::Id)
            .update_columns([
              Column::Age,
              Column::ItemTypeId,
              Column::Luminosity,
              Column::Name,
              Column::Radius,
              Column::SolarSystemId,
              Column::SpectralClass,
              Column::Temperature,
            ])
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
  use pod_model::Star;
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    use sea_orm::ConnectionTrait;
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db.execute_unprepared("PRAGMA foreign_keys = OFF").await.unwrap();
    db
  }

  fn make_star(id: i32, name: &str) -> Star {
    Star::new(id, name)
  }

  mod all {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_stars() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      assert!(repo.all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn returns_all_after_upsert() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_star(1, "Jita Sun")).await.unwrap();
      repo.upsert(&make_star(2, "Amarr Sun")).await.unwrap();
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
      repo.upsert(&make_star(40009077, "Jita Sun")).await.unwrap();
      let result = repo.find(40009077).await.unwrap();
      assert!(result.is_some());
      assert_eq!(*result.unwrap().id(), 40009077);
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
    async fn returns_star_by_name() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_star(1, "Jita Sun")).await.unwrap();
      assert!(repo.find_by_name("Jita Sun").await.unwrap().is_some());
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
    async fn inserts_multiple_stars() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let records = vec![make_star(1, "Star A"), make_star(2, "Star B")];
      repo.upsert_many(&records).await.unwrap();
      assert_eq!(repo.all().await.unwrap().len(), 2);
    }
  }
}
