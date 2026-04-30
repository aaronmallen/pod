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
