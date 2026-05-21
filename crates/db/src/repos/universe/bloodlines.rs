//! Repository for bloodline persistence.

use pod_model::Bloodline;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::bloodline::{ActiveModel, Column, Entity},
};

/// Repository for bloodline CRUD operations.
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

  /// Returns all bloodlines.
  pub async fn all(&self) -> Result<Vec<Bloodline>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Finds a bloodline by its unique ID.
  pub async fn find(&self, id: i32) -> Result<Option<Bloodline>, Error> {
    let row = Entity::find_by_id(id).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Finds a bloodline by its display name (exact match).
  pub async fn find_by_name(&self, name: &str) -> Result<Option<Bloodline>, Error> {
    let row = Entity::find().filter(Column::Name.eq(name)).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Inserts or updates a bloodline row.
  pub async fn upsert(&self, record: &Bloodline) -> Result<(), Error> {
    record.validate()?;
    let active: ActiveModel = record.clone().into();
    Entity::insert(active)
      .on_conflict(
        OnConflict::column(Column::Id)
          .update_columns([
            Column::Charisma,
            Column::CorporationId,
            Column::Description,
            Column::Intelligence,
            Column::Memory,
            Column::Name,
            Column::Perception,
            Column::RaceId,
            Column::ShipItemTypeId,
            Column::WillPower,
          ])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Bulk-upserts bloodline rows in a single batch.
  pub async fn upsert_many(&self, records: &[Bloodline]) -> Result<(), Error> {
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
          .update_columns([
            Column::Charisma,
            Column::CorporationId,
            Column::Description,
            Column::Intelligence,
            Column::Memory,
            Column::Name,
            Column::Perception,
            Column::RaceId,
            Column::ShipItemTypeId,
            Column::WillPower,
          ])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use pod_model::Bloodline;
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    use sea_orm::ConnectionTrait;
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db.execute_unprepared("PRAGMA foreign_keys = OFF").await.unwrap();
    db
  }

  fn make_bloodline(id: i32, name: &str) -> Bloodline {
    Bloodline::new(id, name)
  }

  mod all {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_bloodlines() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.all().await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_all_bloodlines_after_upsert() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_bloodline(1, "Civire")).await.unwrap();
      repo.upsert(&make_bloodline(2, "Deteis")).await.unwrap();
      let result = repo.all().await.unwrap();
      assert_eq!(result.len(), 2);
    }
  }

  mod find {
    use super::*;

    #[tokio::test]
    async fn returns_none_when_not_found() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.find(999).await.unwrap();
      assert!(result.is_none());
    }

    #[tokio::test]
    async fn returns_some_after_upsert() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_bloodline(5, "Jin-Mei")).await.unwrap();
      let result = repo.find(5).await.unwrap();
      assert!(result.is_some());
      assert_eq!(*result.unwrap().id(), 5);
    }
  }

  mod find_by_name {
    use super::*;

    #[tokio::test]
    async fn returns_none_when_not_found() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.find_by_name("Nonexistent").await.unwrap();
      assert!(result.is_none());
    }

    #[tokio::test]
    async fn returns_bloodline_by_exact_name() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_bloodline(1, "Civire")).await.unwrap();
      let result = repo.find_by_name("Civire").await.unwrap();
      assert!(result.is_some());
    }
  }

  mod upsert_many {
    use super::*;

    #[tokio::test]
    async fn does_nothing_when_empty() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert_many(&[]).await.unwrap();
      let result = repo.all().await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn inserts_multiple_bloodlines() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let records = vec![make_bloodline(1, "Civire"), make_bloodline(2, "Deteis")];
      repo.upsert_many(&records).await.unwrap();
      let result = repo.all().await.unwrap();
      assert_eq!(result.len(), 2);
    }
  }
}
