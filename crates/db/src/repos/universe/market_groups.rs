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
