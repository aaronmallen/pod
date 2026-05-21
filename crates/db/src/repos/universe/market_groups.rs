//! Repository for market group persistence.

use pod_model::MarketGroup;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::market_group::{ActiveModel, Column, Entity},
};

/// Repository for market group CRUD operations.
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

  /// Returns all market groups.
  pub async fn all(&self) -> Result<Vec<MarketGroup>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Finds a market group by its unique ID.
  pub async fn find(&self, id: i32) -> Result<Option<MarketGroup>, Error> {
    let row = Entity::find_by_id(id).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Finds a market group by its display name (exact match).
  pub async fn find_by_name(&self, name: &str) -> Result<Option<MarketGroup>, Error> {
    let row = Entity::find().filter(Column::Name.eq(name)).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Inserts or updates a market group row.
  pub async fn upsert(&self, record: &MarketGroup) -> Result<(), Error> {
    record.validate()?;
    let active: ActiveModel = record.clone().into();
    Entity::insert(active)
      .on_conflict(
        OnConflict::column(Column::Id)
          .update_columns([Column::Description, Column::Name, Column::ParentMarketGroupId])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Bulk-upserts market group rows in chunks of 500.
  pub async fn upsert_many(&self, records: &[MarketGroup]) -> Result<(), Error> {
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
            .update_columns([Column::Description, Column::Name, Column::ParentMarketGroupId])
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
  use pod_model::MarketGroup;
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  fn make_market_group(id: i32, name: &str) -> MarketGroup {
    MarketGroup::new(id, name)
  }

  mod all {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_groups() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      assert!(repo.all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn returns_all_after_upsert() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_market_group(1, "Ships")).await.unwrap();
      repo.upsert(&make_market_group(2, "Modules")).await.unwrap();
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
      repo.upsert(&make_market_group(4, "Frigates")).await.unwrap();
      let result = repo.find(4).await.unwrap();
      assert!(result.is_some());
      assert_eq!(*result.unwrap().id(), 4);
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
    async fn returns_group_by_name() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(&make_market_group(4, "Frigates")).await.unwrap();
      assert!(repo.find_by_name("Frigates").await.unwrap().is_some());
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
    async fn inserts_multiple_groups() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let records = vec![make_market_group(1, "Ships"), make_market_group(2, "Modules")];
      repo.upsert_many(&records).await.unwrap();
      assert_eq!(repo.all().await.unwrap().len(), 2);
    }
  }
}
