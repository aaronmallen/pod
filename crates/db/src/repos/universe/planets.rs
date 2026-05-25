//! Repository for planet persistence.

use pod_model::Planet;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::planet::{ActiveModel, Column, Entity},
};

/// Repository for planet CRUD operations.
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

  /// Returns all planets.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn all(&self) -> Result<Vec<Planet>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Finds a planet by its unique ID.
  #[tracing::instrument(level = "trace", skip(self), fields(id = id))]
  pub async fn find(&self, id: i32) -> Result<Option<Planet>, Error> {
    let row = Entity::find_by_id(id).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Finds a planet by its display name (exact match).
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn find_by_name(&self, name: &str) -> Result<Option<Planet>, Error> {
    let row = Entity::find().filter(Column::Name.eq(name)).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Inserts or updates a planet row.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn upsert(&self, record: &Planet) -> Result<(), Error> {
    record.validate()?;
    let active: ActiveModel = record.clone().into();
    Entity::insert(active)
      .on_conflict(
        OnConflict::column(Column::Id)
          .update_columns([
            Column::ItemTypeId,
            Column::Name,
            Column::PositionX,
            Column::PositionY,
            Column::PositionZ,
            Column::SolarSystemId,
          ])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Bulk-upserts planet rows in chunks of 200.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn upsert_many(&self, records: &[Planet]) -> Result<(), Error> {
    if records.is_empty() {
      return Ok(());
    }
    let active = validate_records(records)?;
    for chunk in active.chunks(200) {
      Entity::insert_many(chunk.to_vec())
        .on_conflict(
          OnConflict::column(Column::Id)
            .update_columns([
              Column::ItemTypeId,
              Column::Name,
              Column::PositionX,
              Column::PositionY,
              Column::PositionZ,
              Column::SolarSystemId,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }
}

fn validate_records(records: &[Planet]) -> Result<Vec<ActiveModel>, Error> {
  let mut active = Vec::with_capacity(records.len());
  for record in records {
    record.validate()?;
    active.push(ActiveModel::from(record.clone()));
  }
  Ok(active)
}

#[cfg(test)]
mod tests {
  use pod_model::Planet;
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    use sea_orm::ConnectionTrait;
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db.execute_unprepared("PRAGMA foreign_keys = OFF").await.unwrap();
    db
  }

  fn make_planet(id: i32, name: &str) -> Planet {
    Planet::new(id, name)
  }

  mod all {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_planets() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      assert!(repo.all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn returns_all_after_upsert() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_planet(1, "Jita IV")).await.unwrap();
      repo.upsert(&make_planet(2, "Jita V")).await.unwrap();
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
      repo.upsert(&make_planet(40009086, "Jita IV")).await.unwrap();
      let result = repo.find(40009086).await.unwrap();
      assert!(result.is_some());
      assert_eq!(*result.unwrap().id(), 40009086);
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
    async fn returns_planet_by_name() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_planet(1, "Jita IV")).await.unwrap();
      assert!(repo.find_by_name("Jita IV").await.unwrap().is_some());
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
    async fn inserts_multiple_planets() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let records = vec![make_planet(1, "Planet A"), make_planet(2, "Planet B")];
      repo.upsert_many(&records).await.unwrap();
      assert_eq!(repo.all().await.unwrap().len(), 2);
    }
  }
}
