//! Repository for solar system persistence.

use pod_model::SolarSystem;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::solar_system::{ActiveModel, Column, Entity},
};

/// Repository for solar system CRUD operations.
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

  /// Returns all solar systems.
  pub async fn all(&self) -> Result<Vec<SolarSystem>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Finds a solar system by its unique ID.
  pub async fn find(&self, id: i32) -> Result<Option<SolarSystem>, Error> {
    let row = Entity::find_by_id(id).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Finds a solar system by its display name (exact match).
  pub async fn find_by_name(&self, name: &str) -> Result<Option<SolarSystem>, Error> {
    let row = Entity::find().filter(Column::Name.eq(name)).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Returns raw entity rows for the given solar system IDs.
  pub async fn find_by_ids(&self, ids: &[i32]) -> Result<Vec<crate::entities::solar_system::Model>, Error> {
    let rows = Entity::find()
      .filter(Column::Id.is_in(ids.to_vec()))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Inserts or updates a solar system row.
  pub async fn upsert(&self, record: &SolarSystem) -> Result<(), Error> {
    record.validate()?;
    let active: ActiveModel = record.clone().into();
    Entity::insert(active)
      .on_conflict(
        OnConflict::column(Column::Id)
          .update_columns([
            Column::ConstellationId,
            Column::Name,
            Column::PositionX,
            Column::PositionY,
            Column::PositionZ,
            Column::SecurityClass,
            Column::SecurityStatus,
            Column::StarId,
          ])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Bulk-upserts solar system rows in chunks of 200.
  pub async fn upsert_many(&self, records: &[SolarSystem]) -> Result<(), Error> {
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
              Column::ConstellationId,
              Column::Name,
              Column::PositionX,
              Column::PositionY,
              Column::PositionZ,
              Column::SecurityClass,
              Column::SecurityStatus,
              Column::StarId,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }
}
