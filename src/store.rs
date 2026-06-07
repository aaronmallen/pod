use std::path::Path;

use sqlx::{
  SqlitePool,
  sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

pub mod asset_filter;
pub mod images;
pub mod migration_guard;
pub mod model;
pub mod repo;
pub mod search;

#[derive(Clone, Debug)]
pub struct Database(pub SqlitePool);

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("migration error: {0}")]
  Migration(#[from] sqlx::migrate::MigrateError),
  #[error("the reserved Unassigned squad cannot be created, renamed, or deleted")]
  ReservedSquad,
  #[error("database error: {0}")]
  Sqlx(#[from] sqlx::Error),
}

pub async fn open(path: &Path, network: bool) -> Result<Database, Error> {
  let journal_mode = if network {
    SqliteJournalMode::Delete
  } else {
    SqliteJournalMode::Wal
  };

  let options = SqliteConnectOptions::new()
    .filename(path)
    .create_if_missing(true)
    .foreign_keys(true)
    .journal_mode(journal_mode);

  let pool = SqlitePoolOptions::new().connect_with(options).await?;
  sqlx::migrate!().run(&pool).await?;

  Ok(Database(pool))
}

#[cfg(test)]
pub async fn open_test() -> Result<Database, Error> {
  let pool = SqlitePoolOptions::new()
    .max_connections(1)
    .connect("sqlite::memory:")
    .await?;
  sqlx::migrate!().run(&pool).await?;
  Ok(Database(pool))
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;

  use super::*;

  mod open {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_the_database_file_on_a_fresh_path() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");

      assert!(!path.exists());
      open(&path, false).await.unwrap();

      assert!(path.exists());
    }

    #[tokio::test]
    async fn it_runs_zero_migrations_on_an_existing_schema() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");

      open(&path, false).await.unwrap();
      open(&path, false).await.unwrap();
    }

    #[tokio::test]
    async fn it_sets_delete_journal_mode_for_network_drives() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");

      let db = open(&path, true).await.unwrap();

      let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&db.0)
        .await
        .unwrap();

      assert_eq!(mode, "delete");
    }

    #[tokio::test]
    async fn it_sets_wal_journal_mode_for_local_drives() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");

      let db = open(&path, false).await.unwrap();

      let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&db.0)
        .await
        .unwrap();

      assert_eq!(mode, "wal");
    }
  }
}
