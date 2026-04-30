//! Repository for cached player-owned structure names.

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, sea_query::OnConflict};

use crate::{Error, entities::structure_cache};

/// Repository for structure name cache CRUD operations.
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

  /// Returns cached names for the given structure IDs as a map of id → name.
  pub async fn find_by_ids(&self, ids: &[i64]) -> Result<Vec<(i64, String)>, Error> {
    if ids.is_empty() {
      return Ok(Vec::new());
    }
    let rows = structure_cache::Entity::find()
      .filter(structure_cache::Column::Id.is_in(ids.to_vec()))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(|r| (r.id, r.name)).collect())
  }

  /// Inserts or updates a structure name entry.
  pub async fn upsert(&self, id: i64, name: &str) -> Result<(), Error> {
    let active = structure_cache::ActiveModel {
      id: Set(id),
      name: Set(name.to_string()),
    };
    structure_cache::Entity::insert(active)
      .on_conflict(
        OnConflict::column(structure_cache::Column::Id)
          .update_column(structure_cache::Column::Name)
          .to_owned(),
      )
      .exec(self.db)
      .await?;
    Ok(())
  }

  /// Bulk-upserts structure name entries.
  pub async fn upsert_many(&self, entries: &[(i64, String)]) -> Result<(), Error> {
    if entries.is_empty() {
      return Ok(());
    }
    let active: Vec<structure_cache::ActiveModel> = entries
      .iter()
      .map(|(id, name)| structure_cache::ActiveModel {
        id: Set(*id),
        name: Set(name.clone()),
      })
      .collect();
    for chunk in active.chunks(200) {
      structure_cache::Entity::insert_many(chunk.to_vec())
        .on_conflict(
          OnConflict::column(structure_cache::Column::Id)
            .update_column(structure_cache::Column::Name)
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }
    Ok(())
  }
}
