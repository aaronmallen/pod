//! Repository root and sub-repository modules.

pub mod abyssals;
pub mod assets;
pub mod characters;
pub mod clones;
pub mod contacts;
pub mod corporations;
pub mod killmails;
pub mod notifications;
pub mod prices;
pub mod skill_plans;
pub mod standings;
pub mod stockpiles;
pub mod tags;
mod universe;

use std::path::PathBuf;

use sea_orm::DatabaseConnection;

/// Root repository holding a database connection and providing access to sub-repos.
#[derive(Debug)]
pub struct Root {
  connection: DatabaseConnection,
  lockfile_path: Option<PathBuf>,
}

impl Clone for Root {
  fn clone(&self) -> Self {
    Self {
      connection: self.connection.clone(),
      lockfile_path: None,
    }
  }
}

impl Drop for Root {
  fn drop(&mut self) {
    if let Some(ref path) = self.lockfile_path {
      let _ = std::fs::remove_file(path);
    }
  }
}

impl Root {
  /// Creates a new `Root` repository bound to the given database connection.
  ///
  /// If `lockfile_path` is `Some`, the lockfile at that path is removed when
  /// this repository is dropped.
  pub fn new(connection: DatabaseConnection, lockfile_path: Option<PathBuf>) -> Self {
    Self {
      connection,
      lockfile_path,
    }
  }

  /// Returns a reference to the underlying database connection.
  pub(crate) fn connection(&self) -> &DatabaseConnection {
    &self.connection
  }

  /// Returns an abyssals sub-repository.
  pub fn abyssals(&self) -> abyssals::Repo<'_> {
    abyssals::Repo::new(&self.connection)
  }

  /// Returns an assets sub-repository (corporation assets and sync state).
  pub fn assets(&self) -> assets::Repo<'_> {
    assets::Repo::new(&self.connection)
  }

  /// Returns a characters sub-repository.
  pub fn characters(&self) -> characters::Repo<'_> {
    characters::Repo::new(&self.connection)
  }

  /// Returns a clones sub-repository.
  pub fn clones(&self) -> clones::Repo<'_> {
    clones::Repo::new(&self.connection)
  }

  /// Returns a contacts sub-repository.
  pub fn contacts(&self) -> contacts::Repo<'_> {
    contacts::Repo::new(&self.connection)
  }

  /// Returns a corporations sub-repository.
  pub fn corporations(&self) -> corporations::Repo<'_> {
    corporations::Repo::new(&self.connection)
  }

  /// Returns a killmails sub-repository.
  pub fn killmails(&self) -> killmails::Repo<'_> {
    killmails::Repo::new(&self.connection)
  }

  /// Returns a notifications sub-repository.
  pub fn notifications(&self) -> notifications::Repo<'_> {
    notifications::Repo::new(&self.connection)
  }

  /// Returns a prices sub-repository.
  pub fn prices(&self) -> prices::Repo<'_> {
    prices::Repo::new(&self.connection)
  }

  /// Returns a skill plans sub-repository.
  pub fn skill_plans(&self) -> skill_plans::Repo<'_> {
    skill_plans::Repo::new(&self.connection)
  }

  /// Returns a standings sub-repository.
  pub fn standings(&self) -> standings::Repo<'_> {
    standings::Repo::new(&self.connection)
  }

  /// Returns a stockpiles sub-repository.
  pub fn stockpiles(&self) -> stockpiles::Repo<'_> {
    stockpiles::Repo::new(&self.connection)
  }

  /// Disables SQLite foreign-key enforcement for the current connection.
  pub async fn disable_foreign_keys(&self) -> Result<(), sea_orm::DbErr> {
    use sea_orm::ConnectionTrait;
    self
      .connection
      .execute_unprepared("PRAGMA foreign_keys = OFF")
      .await
      .map(|_| ())
  }

  /// Re-enables SQLite foreign-key enforcement for the current connection.
  pub async fn enable_foreign_keys(&self) -> Result<(), sea_orm::DbErr> {
    use sea_orm::ConnectionTrait;
    self
      .connection
      .execute_unprepared("PRAGMA foreign_keys = ON")
      .await
      .map(|_| ())
  }

  /// Returns a tags sub-repository.
  pub fn tags(&self) -> tags::Repo<'_> {
    tags::Repo::new(&self.connection)
  }

  pub fn universe(&self) -> universe::Repo<'_> {
    universe::Repo::new(&self.connection)
  }
}
