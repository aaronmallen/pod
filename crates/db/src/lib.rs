//! Database layer — entities, domain models, migrations, repos, and schema.

mod entities;
mod migrations;
mod repos;
mod schema;

use std::path::Path;

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
  /// A validation error prevented the operation.
  #[error("validation failed: {0}")]
  Validation(#[from] validator::ValidationErrors),
}

#[tracing::instrument]
pub async fn open(path: &Path) -> Result<Repo, Error> {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).ok();
  }
  let url = format!("sqlite://{}?mode=rwc", path.display());
  let mut options = ConnectOptions::new(url);
  options.map_sqlx_sqlite_opts(|o| o.journal_mode(SqliteJournalMode::Wal));
  let connection = Database::connect(options).await?;
  migrations::run(&connection).await?;
  Ok(Repo::new(connection))
}
