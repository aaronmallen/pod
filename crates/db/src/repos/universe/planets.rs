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
  pub async fn all(&self) -> Result<Vec<Planet>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Finds a planet by its unique ID.
  pub async fn find(&self, id: i32) -> Result<Option<Planet>, Error> {
    let row = Entity::find_by_id(id).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Finds a planet by its display name (exact match).
  pub async fn find_by_name(&self, name: &str) -> Result<Option<Planet>, Error> {
    let row = Entity::find().filter(Column::Name.eq(name)).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Inserts or updates a planet row.
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
  pub async fn upsert_many(&self, records: &[Planet]) -> Result<(), Error> {
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
