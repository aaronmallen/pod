//! Repository for faction persistence.

use pod_model::Faction;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::faction::{ActiveModel, Column, Entity},
};

/// Repository for faction CRUD operations.
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

  /// Returns all factions.
  pub async fn all(&self) -> Result<Vec<Faction>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Finds a faction by its unique ID.
  pub async fn find(&self, id: i32) -> Result<Option<Faction>, Error> {
    let row = Entity::find_by_id(id).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Finds a faction by its display name (exact match).
  pub async fn find_by_name(&self, name: &str) -> Result<Option<Faction>, Error> {
    let row = Entity::find().filter(Column::Name.eq(name)).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Inserts or updates a faction row.
  pub async fn upsert(&self, record: &Faction) -> Result<(), Error> {
    record.validate()?;
    let active: ActiveModel = record.clone().into();
    Entity::insert(active)
      .on_conflict(
        OnConflict::column(Column::Id)
          .update_columns([
            Column::Description,
            Column::IsUnique,
            Column::Name,
            Column::SizeFactor,
            Column::SolarSystemId,
          ])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Bulk-upserts faction rows in a single batch.
  pub async fn upsert_many(&self, records: &[Faction]) -> Result<(), Error> {
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
            Column::Description,
            Column::IsUnique,
            Column::Name,
            Column::SizeFactor,
            Column::SolarSystemId,
          ])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }
}
