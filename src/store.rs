use std::{path::Path, time::Duration};

use sqlx::{
  SqlitePool,
  sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

pub mod asset_filter;
pub mod bootstrap;
pub mod data_restore;
pub mod fs_kind;
pub mod images;
pub mod killmail_slot;
pub mod lease;
mod migration_checksum_repair;
pub mod model;
pub mod reconcile;
pub mod repo;
pub mod search;
pub mod share_meta;
pub mod storage_migration;
pub mod sync_copy;
pub mod sync_session;

const READER_MAX_CONNECTIONS: u32 = 8;

const WRITER_MAX_CONNECTIONS: u32 = 1;

/// Warm the reader pool with a couple of live connections up front so the first interactive read
/// (the cold-open roster load) does not pay connection-establishment + WAL pragma setup latency, and
/// so idle connections are not reaped and re-created in bursts (the source of the observed
/// ~94-connection WAL-pragma storm). `min_connections` keeps these alive for the process lifetime.
const READER_MIN_CONNECTIONS: u32 = 2;

const WRITER_MIN_CONNECTIONS: u32 = 1;

const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

/// Per-connection page cache. The seed transaction still benefits from a generous cache, but the old
/// 256MB/connection (`-262144`) was wildly oversized: with the reader pool it could reserve gigabytes
/// of resident memory. 48MB/connection (`-49152`, in KiB) keeps large seeds and roster reads in
/// memory while bounding worst-case resident cache to a few hundred MB across the whole reader pool.
const CACHE_SIZE_PRAGMA: &str = "-49152";

#[derive(Clone, Debug)]
pub struct Database(pub SqlitePool, pub SqlitePool);

impl Database {
  #[must_use]
  pub fn reader(&self) -> &SqlitePool {
    &self.0
  }

  #[must_use]
  pub fn writer(&self) -> &SqlitePool {
    &self.1
  }
}

/// SQLite extended result code for `SQLITE_CONSTRAINT_FOREIGNKEY` (787), distinct from the primary
/// `SQLITE_CONSTRAINT` code (19). sqlx surfaces it as a string via `DatabaseError::code`.
const SQLITE_FOREIGN_KEY_CODE: &str = "787";

/// SQLite extended result code for `SQLITE_CONSTRAINT_UNIQUE` (2067), distinct from the primary
/// `SQLITE_CONSTRAINT` code (19). sqlx surfaces it as a string via `DatabaseError::code`.
const SQLITE_UNIQUE_CODE: &str = "2067";

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error(
    "foreign key violation persisting {context} (a referenced org row was not inserted in the same transaction): {source}"
  )]
  ForeignKey {
    context: String,
    #[source]
    source: sqlx::Error,
  },
  #[error("migration error: {0}")]
  Migration(#[from] sqlx::migrate::MigrateError),
  #[error("the reserved Unassigned squad cannot be created, renamed, or deleted")]
  ReservedSquad,
  #[error("a tag named {name:?} already exists in this scope")]
  TagNameTaken { name: String },
  #[error("database error: {0}")]
  Sqlx(#[from] sqlx::Error),
}

pub struct Pools {
  pub housekeeping: Database,
  pub interactive: Database,
  pub sync: Database,
}

impl Error {
  pub fn is_foreign_key_violation(&self) -> bool {
    match self {
      Error::ForeignKey {
        ..
      } => true,
      Error::Sqlx(source) => is_foreign_key_constraint(source),
      _ => false,
    }
  }

  // Consumed by the tag find-or-create / rename UI; exercised by unit tests until that caller lands.
  #[cfg_attr(not(test), expect(dead_code))]
  pub fn is_unique_violation(&self) -> bool {
    match self {
      Error::Sqlx(source) => is_unique_constraint(source),
      _ => false,
    }
  }
}

pub(crate) fn is_foreign_key_constraint(error: &sqlx::Error) -> bool {
  error
    .as_database_error()
    .and_then(|db| db.code())
    .is_some_and(|code| code == SQLITE_FOREIGN_KEY_CODE)
}

pub(crate) fn is_unique_constraint(error: &sqlx::Error) -> bool {
  error
    .as_database_error()
    .and_then(|db| db.code())
    .is_some_and(|code| code == SQLITE_UNIQUE_CODE)
}

pub async fn open(path: &Path) -> Result<Database, Error> {
  let writer = connect_pool(path, WRITER_MAX_CONNECTIONS, WRITER_MIN_CONNECTIONS).await?;
  let migrator = sqlx::migrate!();
  // Before sqlx validates checksums, heal the CRLF↔LF migration-checksum drift that pre-0.6.7
  // Windows builds recorded; otherwise that validation refuses to open the (intact) database.
  // No-op on healthy databases and fresh installs. See `migration_checksum_repair`.
  let healed = migration_checksum_repair::repair_crlf_checksums(&writer, &migrator).await?;
  if healed > 0 {
    tracing::info!(target: "pod::lifecycle", healed = healed as u64, "repaired CRLF migration checksums");
  }
  migrator.run(&writer).await?;

  let reader = connect_pool(path, READER_MAX_CONNECTIONS, READER_MIN_CONNECTIONS).await?;

  Ok(Database(reader, writer))
}

pub async fn open_pools(path: &Path) -> Result<Pools, Error> {
  let database = open(path).await?;
  Ok(Pools {
    housekeeping: database.clone(),
    interactive: database.clone(),
    sync: database,
  })
}

async fn connect_pool(path: &Path, max_connections: u32, min_connections: u32) -> Result<SqlitePool, sqlx::Error> {
  let options = SqliteConnectOptions::new()
    .filename(path)
    .create_if_missing(true)
    .foreign_keys(true)
    .journal_mode(SqliteJournalMode::Wal)
    .synchronous(SqliteSynchronous::Normal)
    .pragma("cache_size", CACHE_SIZE_PRAGMA)
    .pragma("temp_store", "MEMORY")
    .busy_timeout(ACQUIRE_TIMEOUT);

  SqlitePoolOptions::new()
    .max_connections(max_connections)
    .min_connections(min_connections)
    .acquire_timeout(ACQUIRE_TIMEOUT)
    .connect_with(options)
    .await
}

#[cfg(test)]
pub async fn open_test() -> Result<Database, Error> {
  let pool = SqlitePoolOptions::new()
    .max_connections(1)
    .connect("sqlite::memory:")
    .await?;
  sqlx::migrate!().run(&pool).await?;
  Ok(Database(pool.clone(), pool))
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;

  use super::*;

  mod open {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_always_opens_in_wal_mode() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");

      let db = open(&path).await.unwrap();

      let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(db.reader())
        .await
        .unwrap();

      assert_eq!(mode, "wal");
    }

    #[tokio::test]
    async fn it_applies_the_write_path_pragmas_that_keep_large_seeds_off_the_disk() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");

      let db = open(&path).await.unwrap();

      let cache_size: i64 = sqlx::query_scalar("PRAGMA cache_size")
        .fetch_one(db.writer())
        .await
        .unwrap();
      let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
        .fetch_one(db.writer())
        .await
        .unwrap();
      let temp_store: i64 = sqlx::query_scalar("PRAGMA temp_store")
        .fetch_one(db.writer())
        .await
        .unwrap();

      assert_eq!(
        cache_size, -49152,
        "48MB page cache holds the seed transaction in memory without oversizing per-connection RAM"
      );
      assert_eq!(synchronous, 1, "NORMAL synchronous (1) trims fsync round-trips");
      assert_eq!(temp_store, 2, "MEMORY temp store (2) keeps temp b-trees off the drive");
    }

    #[tokio::test]
    async fn it_caps_the_writer_pool_at_a_single_connection() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");

      let db = open(&path).await.unwrap();

      let held = db.writer().acquire().await.unwrap();

      let second_writer = tokio::time::timeout(Duration::from_millis(250), db.writer().acquire()).await;
      assert!(
        second_writer.is_err(),
        "the writer pool is capped at one connection, so a second writer acquire must not be served"
      );

      let reader = tokio::time::timeout(Duration::from_millis(250), db.reader().acquire()).await;
      assert!(
        reader.is_ok(),
        "reads must be served from the reader pool even while the single writer connection is held"
      );

      drop(held);
    }

    #[tokio::test]
    async fn it_serves_reads_under_simulated_sync_write_pressure() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");
      let db = open(&path).await.unwrap();

      sqlx::query("CREATE TABLE pressure (id INTEGER PRIMARY KEY, n INTEGER NOT NULL)")
        .execute(db.writer())
        .await
        .unwrap();

      let writer = db.clone();
      let storm = tokio::spawn(async move {
        for i in 0..200_i64 {
          let mut tx = writer.writer().begin().await.unwrap();
          sqlx::query("INSERT INTO pressure (n) VALUES (?)")
            .bind(i)
            .execute(&mut *tx)
            .await
            .unwrap();
          tx.commit().await.unwrap();
        }
      });

      for _ in 0..50 {
        let read = tokio::time::timeout(
          ACQUIRE_TIMEOUT,
          sqlx::query_scalar::<_, i64>("SELECT count(*) FROM pressure").fetch_one(db.reader()),
        )
        .await;
        assert!(
          read.is_ok(),
          "reads must never block on the write lock or hit the acquire timeout under write pressure"
        );
        read.unwrap().unwrap();
      }

      storm.await.unwrap();
    }

    #[tokio::test]
    async fn it_creates_the_database_file_on_a_fresh_path() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");

      assert!(!path.exists());
      open(&path).await.unwrap();

      assert!(path.exists());
    }

    #[tokio::test]
    async fn it_runs_zero_migrations_on_an_existing_schema() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");

      open(&path).await.unwrap();
      open(&path).await.unwrap();
    }
  }

  mod open_pools {
    use super::*;

    #[tokio::test]
    async fn it_opens_one_writer_and_a_shared_reader_pool_over_one_migrated_database() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");

      let pools = super::super::open_pools(&path).await.unwrap();

      for db in [&pools.interactive, &pools.sync, &pools.housekeeping] {
        let migrations: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
          .fetch_one(db.reader())
          .await
          .unwrap();
        assert!(
          migrations > 0,
          "every handle reads the schema open_pools migrated, applied exactly once"
        );
      }

      let held = pools.interactive.writer().acquire().await.unwrap();
      let other = tokio::time::timeout(Duration::from_millis(250), pools.sync.writer().acquire()).await;
      assert!(
        other.is_err(),
        "all handles share one writer connection, so a write through another handle cannot proceed concurrently"
      );
      drop(held);
    }
  }

  mod is_unique_violation {
    use super::*;

    #[tokio::test]
    async fn it_classifies_a_2067_as_a_unique_violation() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");
      let db = open(&path).await.unwrap();

      sqlx::query("INSERT INTO tags (color, created_at, description, name, position, scope, updated_at) VALUES (NULL, 0, NULL, 'Roller', 0, 'asset', 0)")
        .execute(db.writer())
        .await
        .unwrap();
      let error: Error = sqlx::query("INSERT INTO tags (color, created_at, description, name, position, scope, updated_at) VALUES (NULL, 0, NULL, 'roller', 1, 'asset', 0)")
        .execute(db.writer())
        .await
        .expect_err("a case-insensitive duplicate within one scope violates the unique index")
        .into();

      assert!(
        error.is_unique_violation(),
        "the 2067 is classified as a unique violation"
      );
      assert!(
        !error.is_foreign_key_violation(),
        "a unique violation is not a foreign-key violation"
      );
    }
  }

  mod rekey_wallet_journal_per_wallet {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn primary_key_columns(db: &Database, table: &str) -> Vec<String> {
      sqlx::query_scalar::<_, String>("SELECT name FROM pragma_table_info(?) WHERE pk > 0 ORDER BY pk")
        .bind(table)
        .fetch_all(db.reader())
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn it_applies_cleanly_and_rekeys_every_wallet_table_to_the_composite_identity() {
      let db = open_test().await.unwrap();

      assert_eq!(
        primary_key_columns(&db, "character_wallet_journal").await,
        vec!["character_id".to_owned(), "id".to_owned()]
      );
      assert_eq!(
        primary_key_columns(&db, "character_wallet_transaction").await,
        vec!["character_id".to_owned(), "transaction_id".to_owned()]
      );
      assert_eq!(
        primary_key_columns(&db, "corporation_wallet_journal").await,
        vec!["corporation_id".to_owned(), "division".to_owned(), "id".to_owned()]
      );
      assert_eq!(
        primary_key_columns(&db, "corporation_wallet_transaction").await,
        vec![
          "corporation_id".to_owned(),
          "division".to_owned(),
          "transaction_id".to_owned()
        ]
      );
    }
  }
}
