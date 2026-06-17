use std::{path::Path, time::Duration};

use sqlx::{
  SqlitePool,
  sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

pub mod asset_filter;
pub mod bootstrap;
pub mod fs_kind;
pub mod images;
pub mod killmail_slot;
pub mod lease;
pub mod model;
pub mod reconcile;
pub mod repo;
pub mod search;
pub mod share_meta;
pub mod storage_migration;
pub mod sync_copy;
pub mod sync_session;

const HOUSEKEEPING_MAX_CONNECTIONS: u32 = 1;

const INTERACTIVE_MAX_CONNECTIONS: u32 = 4;

const SYNC_MAX_CONNECTIONS: u32 = 4;

#[derive(Clone, Debug)]
pub struct Database(pub SqlitePool);

/// SQLite extended result code for `SQLITE_CONSTRAINT_FOREIGNKEY` (787), distinct from the primary
/// `SQLITE_CONSTRAINT` code (19). sqlx surfaces it as a string via `DatabaseError::code`.
const SQLITE_FOREIGN_KEY_CODE: &str = "787";

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
  #[error("database error: {0}")]
  Sqlx(#[from] sqlx::Error),
}

/// The trio of pools the app runs against a single database file: the interactive pool that serves
/// auth/roster reads, the sync worker pool, and the single-connection housekeeping pool. See the
/// individual `open*` functions for why each exists.
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
}

pub(crate) fn is_foreign_key_constraint(error: &sqlx::Error) -> bool {
  error
    .as_database_error()
    .and_then(|db| db.code())
    .is_some_and(|code| code == SQLITE_FOREIGN_KEY_CODE)
}

pub async fn open(path: &Path) -> Result<Database, Error> {
  let pool = connect_pool(path, INTERACTIVE_MAX_CONNECTIONS).await?;
  sqlx::migrate!().run(&pool).await?;

  Ok(Database(pool))
}

/// Opens all three app pools over one database file. `open` runs migrations once; the sync and
/// housekeeping pools assume the schema is already current and rerun none.
pub async fn open_pools(path: &Path) -> Result<Pools, Error> {
  Ok(Pools {
    interactive: open(path).await?,
    sync: open_sync_pool(path).await?,
    housekeeping: open_housekeeping_pool(path).await?,
  })
}

/// Opens a second pool over the same database, dedicated to sync workers so interactive connections
/// never queue behind them. Assumes `open` already ran migrations, and deliberately reruns none.
pub async fn open_sync_pool(path: &Path) -> Result<Database, Error> {
  Ok(Database(connect_pool(path, SYNC_MAX_CONNECTIONS).await?))
}

/// Opens a tiny single-connection pool over the same database, reserved exclusively for the sync
/// engine's housekeeping writes (the ledger upsert after every job plus the outbox drain/prune
/// maintenance). Keeping it off the worker pool means a connection is always free for housekeeping
/// even when every worker holds one, so the engine's finish loop never queues behind the
/// `busy_timeout` for a connection slot. SQLite is still single-writer, so this never raises the
/// number of concurrent writers — it only stops the workers from starving housekeeping of a slot.
/// Assumes `open` already ran migrations, and deliberately reruns none.
pub async fn open_housekeeping_pool(path: &Path) -> Result<Database, Error> {
  Ok(Database(connect_pool(path, HOUSEKEEPING_MAX_CONNECTIONS).await?))
}

async fn connect_pool(path: &Path, max_connections: u32) -> Result<SqlitePool, sqlx::Error> {
  let options = SqliteConnectOptions::new()
    .filename(path)
    .create_if_missing(true)
    .foreign_keys(true)
    .journal_mode(SqliteJournalMode::Wal)
    // A 2MB default page cache forces the large seed transactions to spill
    // dirty pages mid-flight, each a slow synchronous write. A 256MB cache
    // keeps the seed in memory and flushes once at commit. NORMAL synchronous
    // and an in-memory temp store cut the remaining fsync round-trips. See
    // store::open tests for the assertions.
    .synchronous(SqliteSynchronous::Normal)
    .pragma("cache_size", "-262144")
    .pragma("temp_store", "MEMORY")
    .busy_timeout(Duration::from_secs(15));

  // SQLite is single-writer, so a small pool is plenty; capping it bounds the
  // worst-case resident cache (256MB per connection) the pragmas above allow.
  SqlitePoolOptions::new()
    .max_connections(max_connections)
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

    #[tokio::test]
    async fn it_always_opens_in_wal_mode() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");

      let db = open(&path).await.unwrap();

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

      let db = open(&path).await.unwrap();

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

  mod open_pools {
    use super::*;

    #[tokio::test]
    async fn it_opens_all_three_pools_over_one_migrated_database() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");

      let pools = super::super::open_pools(&path).await.unwrap();

      for pool in [&pools.interactive, &pools.sync, &pools.housekeeping] {
        let migrations: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
          .fetch_one(&pool.0)
          .await
          .unwrap();
        assert!(
          migrations > 0,
          "every pool sees the schema open_pools migrated, applied exactly once"
        );
      }
    }
  }

  mod open_sync_pool {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_serves_the_already_migrated_schema_from_a_separate_pool() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");
      open(&path).await.unwrap();

      let sync = open_sync_pool(&path).await.unwrap();

      let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&sync.0)
        .await
        .unwrap();
      assert_eq!(mode, "wal", "the sync pool opens the same WAL database");

      let migrations: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&sync.0)
        .await
        .unwrap();
      assert!(
        migrations > 0,
        "the sync pool sees the migrations open() applied, and reruns none"
      );
    }
  }

  mod open_housekeeping_pool {
    use super::*;

    #[tokio::test]
    async fn it_serves_a_connection_while_every_sync_worker_connection_is_held() {
      let dir = tempdir().unwrap();
      let path = dir.path().join("test.db");
      open(&path).await.unwrap();
      let sync = open_sync_pool(&path).await.unwrap();
      let housekeeping = open_housekeeping_pool(&path).await.unwrap();

      // Saturate the worker pool: hold every one of its connections, mirroring MAX_CONCURRENT_JOBS
      // workers each mid-write. The housekeeping pool is a separate set of connections, so it must
      // still hand one out immediately rather than queue behind the busy worker pool.
      let mut held = Vec::new();
      for _ in 0..SYNC_MAX_CONNECTIONS {
        held.push(sync.0.acquire().await.unwrap());
      }

      let connection = tokio::time::timeout(Duration::from_secs(1), housekeeping.0.acquire()).await;

      assert!(
        connection.is_ok(),
        "housekeeping must get a connection even when all {SYNC_MAX_CONNECTIONS} worker connections are held"
      );
    }
  }
}
