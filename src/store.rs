use std::{path::Path, time::Duration};

use sqlx::{
  SqlitePool,
  sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

pub mod asset_filter;
pub mod fs_kind;
pub mod images;
pub mod migration_guard;
pub mod model;
pub mod repo;
pub mod search;
pub mod share_meta;

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
    .journal_mode(journal_mode)
    // A 2MB default page cache forces the large seed transactions to spill
    // dirty pages mid-flight; on a network drive each spill is many slow,
    // synchronous writes. A 256MB cache keeps the seed in memory and flushes
    // once at commit. NORMAL synchronous and an in-memory temp store cut the
    // remaining fsync round-trips. See store::open tests for the assertions.
    .synchronous(SqliteSynchronous::Normal)
    .pragma("cache_size", "-262144")
    .pragma("temp_store", "MEMORY")
    .busy_timeout(Duration::from_secs(15));

  // SQLite is single-writer, so a small pool is plenty; capping it bounds the
  // worst-case resident cache (256MB per connection) the pragmas above allow.
  let pool = SqlitePoolOptions::new()
    .max_connections(4)
    .connect_with(options)
    .await?;
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

    #[tokio::test]
    async fn it_applies_the_write_path_pragmas_that_keep_large_seeds_off_the_disk() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");

      let db = open(&path, true).await.unwrap();

      let cache_size: i64 = sqlx::query_scalar("PRAGMA cache_size").fetch_one(&db.0).await.unwrap();
      let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous").fetch_one(&db.0).await.unwrap();
      let temp_store: i64 = sqlx::query_scalar("PRAGMA temp_store").fetch_one(&db.0).await.unwrap();

      assert_eq!(
        cache_size, -262144,
        "256MB page cache holds the seed transaction in memory"
      );
      assert_eq!(synchronous, 1, "NORMAL synchronous (1) trims fsync round-trips");
      assert_eq!(temp_store, 2, "MEMORY temp store (2) keeps temp b-trees off the drive");
    }
  }
}
