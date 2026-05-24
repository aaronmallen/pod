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

  /// Returns cached entries for the given structure IDs as `(id, name, solar_system_id)` triples.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn find_by_ids(&self, ids: &[i64]) -> Result<Vec<(i64, String, Option<i64>)>, Error> {
    if ids.is_empty() {
      return Ok(Vec::new());
    }
    let rows = structure_cache::Entity::find()
      .filter(structure_cache::Column::Id.is_in(ids.to_vec()))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(|r| (r.id, r.name, r.solar_system_id)).collect())
  }

  /// Inserts or updates a structure name entry.
  #[tracing::instrument(level = "trace", skip(self), fields(id = id))]
  pub async fn upsert(&self, id: i64, name: &str) -> Result<(), Error> {
    let active = structure_cache::ActiveModel {
      id: Set(id),
      name: Set(name.to_string()),
      solar_system_id: Set(None),
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

  /// Bulk-upserts structure name entries with solar system IDs.
  #[tracing::instrument(level = "trace", skip(self))]
  pub async fn upsert_many(&self, entries: &[(i64, String, Option<i64>)]) -> Result<(), Error> {
    if entries.is_empty() {
      return Ok(());
    }
    let active: Vec<structure_cache::ActiveModel> = entries
      .iter()
      .map(|(id, name, sys_id)| structure_cache::ActiveModel {
        id: Set(*id),
        name: Set(name.clone()),
        solar_system_id: Set(*sys_id),
      })
      .collect();
    for chunk in active.chunks(200) {
      structure_cache::Entity::insert_many(chunk.to_vec())
        .on_conflict(
          OnConflict::column(structure_cache::Column::Id)
            .update_columns([structure_cache::Column::Name, structure_cache::Column::SolarSystemId])
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
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  mod find_by_ids {
    use super::*;

    #[tokio::test]
    async fn returns_empty_for_empty_ids() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      assert!(repo.find_by_ids(&[]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn returns_matching_structures() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(1000000000001, "Azbel One").await.unwrap();
      repo.upsert(1000000000002, "Raitaru Two").await.unwrap();

      let result = repo.find_by_ids(&[1000000000001]).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].0, 1000000000001);
      assert_eq!(result[0].1, "Azbel One");
    }

    #[tokio::test]
    async fn returns_empty_when_ids_not_found() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.find_by_ids(&[9999999999]).await.unwrap();
      assert!(result.is_empty());
    }
  }

  mod upsert {
    use super::*;

    #[tokio::test]
    async fn inserts_structure_with_no_solar_system() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(1000000000001, "My Citadel").await.unwrap();

      let result = repo.find_by_ids(&[1000000000001]).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].2, None);
    }

    #[tokio::test]
    async fn updates_name_on_conflict() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert(1000000000001, "Old Name").await.unwrap();
      repo.upsert(1000000000001, "New Name").await.unwrap();

      let result = repo.find_by_ids(&[1000000000001]).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0].1, "New Name");
    }
  }

  mod upsert_many {
    use super::*;

    #[tokio::test]
    async fn does_nothing_when_empty() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.upsert_many(&[]).await.unwrap();
      let result = repo.find_by_ids(&[1]).await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn inserts_multiple_entries_with_solar_system_ids() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let entries = vec![
        (1000000000001_i64, "Azbel One".to_string(), Some(30000142_i64)),
        (1000000000002_i64, "Raitaru Two".to_string(), None),
      ];
      repo.upsert_many(&entries).await.unwrap();

      let result = repo.find_by_ids(&[1000000000001, 1000000000002]).await.unwrap();
      assert_eq!(result.len(), 2);
    }
  }
}
