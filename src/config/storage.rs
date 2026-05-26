//! Storage path overrides for database, cache, and log directories.

use std::path::PathBuf;

use getset::Getters;
use serde::{Deserialize, Serialize};

/// User-configurable overrides for Pod's on-disk storage locations.
///
/// All fields are `None` by default, meaning Pod falls back to its
/// platform-standard directories.
#[derive(Clone, Debug, Default, Deserialize, Getters, Serialize)]
#[serde(default)]
pub struct Settings {
  /// Custom directory for the SQLite database (`pod.db`).
  #[getset(get = "pub")]
  db_dir: Option<PathBuf>,

  /// Custom root for the ESI HTTP cache.
  #[getset(get = "pub")]
  cache_dir: Option<PathBuf>,

  /// Custom directory for rolling log files.
  #[getset(get = "pub")]
  log_dir: Option<PathBuf>,
}

impl Settings {
  /// Set the database directory override.
  pub fn set_db_dir(&mut self, path: Option<PathBuf>) {
    self.db_dir = path;
  }

  /// Set the cache directory override.
  pub fn set_cache_dir(&mut self, path: Option<PathBuf>) {
    self.cache_dir = path;
  }

  /// Set the log directory override.
  pub fn set_log_dir(&mut self, path: Option<PathBuf>) {
    self.log_dir = path;
  }
}
