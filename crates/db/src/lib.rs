//! Database layer — entities, domain models, migrations, repos, and schema.

mod entities;
mod migrations;
mod repos;
mod schema;

use std::path::{Path, PathBuf};

pub use repos::{
  Root as Repo,
  clones::{StartupClone, StartupImplant},
  stockpiles::{StockpileItem, StockpileItemStatus, StockpileWithItems},
};
use sea_orm::{ConnectOptions, Database, sqlx::sqlite::SqliteJournalMode};

/// Error type for all database operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// A database error occurred.
  #[error("database error: {0}")]
  Database(#[from] sea_orm::DbErr),
  /// An I/O error occurred while managing the lockfile.
  #[error("lockfile error: {0}")]
  Lockfile(#[from] std::io::Error),
  /// A validation error prevented the operation.
  #[error("validation failed: {0}")]
  Validation(#[from] validator::ValidationErrors),
}

/// The result of successfully opening a database.
///
/// Carries the connected repository and an optional warning that another
/// host already holds the lockfile. The `Repo` is fully usable regardless
/// of whether `existing_lock_host` is set.
pub struct OpenResult {
  /// The connected repository.
  pub repo: Repo,
  /// The hostname recorded in an existing lockfile, if the lockfile was
  /// written by a different host. `None` when there is no conflict.
  pub existing_lock_host: Option<String>,
}

/// Opens (or creates) the SQLite database at `path`, runs pending migrations,
/// and returns an [`OpenResult`].
///
/// When `network_db` is `true` the connection uses `journal_mode=DELETE`
/// instead of WAL so the file can live on a network share. When `false`
/// WAL mode is used for optimal local performance.
///
/// A lockfile at `<path>.lock` is written with `<hostname>:<pid>`. If a
/// lockfile from a **different** host already exists a warning is logged and
/// the conflicting hostname is returned in [`OpenResult::existing_lock_host`].
/// Same-host conflicts are silently overwritten. The lockfile is removed when
/// the returned [`Repo`] is dropped.
#[tracing::instrument]
pub async fn open(path: &Path, network_db: bool) -> Result<OpenResult, Error> {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).ok();
  }

  let lockfile_path = lockfile_path_for(path);
  let existing_lock_host = manage_lockfile(&lockfile_path)?;

  let url = format!("sqlite://{}?mode=rwc", path.display());
  let mut options = ConnectOptions::new(url);
  if network_db {
    options.map_sqlx_sqlite_opts(|o| o.journal_mode(SqliteJournalMode::Delete));
  } else {
    options.map_sqlx_sqlite_opts(|o| o.journal_mode(SqliteJournalMode::Wal));
  }
  let connection = Database::connect(options).await?;
  migrations::run(&connection).await?;

  let repo = Repo::new(connection, Some(lockfile_path));
  Ok(OpenResult {
    repo,
    existing_lock_host,
  })
}

/// Returns the lockfile path for a given database path.
fn lockfile_path_for(db_path: &Path) -> PathBuf {
  let mut p = db_path.to_owned();
  let file_name = p
    .file_name()
    .map(|n| {
      let mut s = n.to_os_string();
      s.push(".lock");
      s
    })
    .unwrap_or_else(|| "pod.db.lock".into());
  p.set_file_name(file_name);
  p
}

/// Writes `<hostname>:<pid>` to the lockfile.
///
/// If an existing lockfile from a **different** hostname is present, logs a
/// warning and returns that hostname. Same-host lockfiles are silently
/// overwritten.
fn manage_lockfile(lockfile_path: &Path) -> Result<Option<String>, std::io::Error> {
  let hostname = gethostname::gethostname().to_string_lossy().into_owned();
  let pid = std::process::id();
  let entry = format!("{hostname}:{pid}");

  let mut existing_lock_host: Option<String> = None;

  if lockfile_path.exists() {
    if let Ok(contents) = std::fs::read_to_string(lockfile_path) {
      let existing_host = contents.splitn(2, ':').next().unwrap_or("").to_owned();
      if !existing_host.is_empty() && existing_host != hostname {
        tracing::warn!(
          lockfile = %lockfile_path.display(),
          host = %existing_host,
          "database lockfile held by another host"
        );
        existing_lock_host = Some(existing_host);
      }
    }
  }

  std::fs::write(lockfile_path, &entry)?;
  Ok(existing_lock_host)
}

#[cfg(test)]
mod tests {
  use super::*;

  mod open {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    use super::*;

    #[tokio::test]
    async fn it_uses_delete_journal_mode_when_network_db_is_true() {
      use pretty_assertions::assert_eq;

      let db_path = std::env::temp_dir().join("pod_db_journal_mode_test.db");

      let result = open(&db_path, true).await.unwrap();

      let stmt = Statement::from_string(DatabaseBackend::Sqlite, "PRAGMA journal_mode;");
      let row = result.repo.connection().query_one_raw(stmt).await.unwrap().unwrap();
      let mode: String = row.try_get("", "journal_mode").unwrap();

      assert_eq!(mode, "delete");
    }
  }
}
