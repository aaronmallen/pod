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
  /// Custom root for the ESI HTTP cache.
  #[getset(get = "pub")]
  cache_dir: Option<PathBuf>,

  /// Custom directory for the SQLite database (`pod.db`).
  #[getset(get = "pub")]
  db_dir: Option<PathBuf>,

  /// Custom directory for rolling log files.
  #[getset(get = "pub")]
  log_dir: Option<PathBuf>,

  /// Whether to use a networked database instead of a local SQLite file.
  ///
  /// Defaults to `false` when absent from the config file so existing
  /// configurations continue to use the local SQLite database.
  network_db: Option<bool>,
}

impl Settings {
  /// Returns `true` if a networked database should be used.
  ///
  /// Falls back to `false` when the field is absent from the config.
  pub fn resolved_network_db(&self) -> bool {
    self.network_db.unwrap_or(false)
  }

  /// Set the cache directory override.
  pub fn set_cache_dir(&mut self, path: Option<PathBuf>) {
    self.cache_dir = path;
  }

  /// Set the database directory override.
  pub fn set_db_dir(&mut self, path: Option<PathBuf>) {
    self.db_dir = path;
  }

  /// Set the log directory override.
  pub fn set_log_dir(&mut self, path: Option<PathBuf>) {
    self.log_dir = path;
  }

  /// Set the network database flag.
  pub fn set_network_db(&mut self, value: bool) {
    self.network_db = Some(value);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod settings {
    use super::*;

    mod resolved_network_db {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_false_when_field_is_none() {
        let settings = Settings::default();

        assert_eq!(settings.resolved_network_db(), false);
      }

      #[test]
      fn it_returns_true_when_field_is_some_true() {
        let mut settings = Settings::default();
        settings.network_db = Some(true);

        assert_eq!(settings.resolved_network_db(), true);
      }

      #[test]
      fn it_returns_false_when_field_is_some_false() {
        let mut settings = Settings::default();
        settings.network_db = Some(false);

        assert_eq!(settings.resolved_network_db(), false);
      }
    }
  }
}
