//! Repository for item category persistence.

use pod_model::ItemCategory;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, sea_query::OnConflict};
use validator::Validate;

use crate::{
  Error,
  entities::item_category::{ActiveModel, Column, Entity},
};

/// Repository for item category CRUD operations.
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

  /// Returns all item categories.
  pub async fn all(&self) -> Result<Vec<ItemCategory>, Error> {
    let rows = Entity::find().all(self.db).await?;
    Ok(rows.into_iter().map(Into::into).collect())
  }

  /// Finds an item category by its unique ID.
  pub async fn find(&self, id: i32) -> Result<Option<ItemCategory>, Error> {
    let row = Entity::find_by_id(id).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Finds an item category by its display name (exact match).
  pub async fn find_by_name(&self, name: &str) -> Result<Option<ItemCategory>, Error> {
    let row = Entity::find().filter(Column::Name.eq(name)).one(self.db).await?;
    Ok(row.map(Into::into))
  }

  /// Returns raw entity rows for the given category IDs.
  pub async fn find_by_ids(&self, ids: &[i32]) -> Result<Vec<crate::entities::item_category::Model>, Error> {
    let rows = Entity::find()
      .filter(Column::Id.is_in(ids.to_vec()))
      .all(self.db)
      .await?;
    Ok(rows)
  }

  /// Inserts or updates an item category row.
  pub async fn upsert(&self, record: &ItemCategory) -> Result<(), Error> {
    record.validate()?;
    let active: ActiveModel = record.clone().into();
    Entity::insert(active)
      .on_conflict(
        OnConflict::column(Column::Id)
          .update_columns([Column::Name, Column::Published])
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Bulk-upserts item category rows in chunks of 500.
  pub async fn upsert_many(&self, records: &[ItemCategory]) -> Result<(), Error> {
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
            .update_columns([Column::Name, Column::Published])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }
}
