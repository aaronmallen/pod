//! Application configuration loading and persistence.

pub mod features;
pub mod storage;

use std::{path::PathBuf, sync::OnceLock};

use figment::{
  Figment,
  providers::{Format, Serialized, Toml},
};
use getset::Getters;
use serde::{Deserialize, Serialize};

static GLOBAL: OnceLock<Settings> = OnceLock::new();

/// Error variants for config load failures.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// No user config directory could be located on this platform.
  #[error("failed to determine user's config directory")]
  ConfigDirNotFound,
  /// Figment failed to extract the configuration.
  #[error(transparent)]
  Load(#[from] Box<figment::Error>),
}

/// Top-level application configuration.
#[derive(Clone, Debug, Default, Deserialize, Getters, Serialize)]
pub struct Settings {
  /// Feature flag configuration.
  #[getset(get = "pub")]
  #[serde(default)]
  features: features::Settings,

  /// Storage path overrides.
  #[getset(get = "pub")]
  #[serde(default)]
  storage: storage::Settings,
}

impl Settings {
  /// Returns the global `Settings` instance, which must have been
  /// initialized with [`init_global`] before this is called.
  ///
  /// Panics if called before [`init_global`].
  pub fn global() -> &'static Settings {
    GLOBAL.get().expect("Config::global() called before init_global()")
  }

  /// Returns the resolved ESI disk-cache directory.
  ///
  /// Uses `storage.cache_dir` if set; otherwise falls back to
  /// `{cache_home}/pod`.
  pub fn resolved_cache_dir(&self) -> PathBuf {
    self.storage.cache_dir().clone().unwrap_or_else(|| {
      dir_spec::cache_home()
        .map(|p| p.join("pod"))
        .expect("failed to resolve cache directory")
    })
  }

  /// Returns the resolved SQLite database path.
  ///
  /// Uses `storage.db_dir` as the file path if set; otherwise falls
  /// back to `{data_home}/pod/pod.db`.
  pub fn resolved_db_path(&self) -> PathBuf {
    self.storage.db_dir().clone().unwrap_or_else(|| {
      dir_spec::data_home()
        .map(|p| p.join("pod").join("pod.db"))
        .expect("failed to resolve user data directory")
    })
  }

  /// Returns the resolved rolling log directory.
  ///
  /// Uses `storage.log_dir` if set; otherwise falls back to
  /// `{state_home}/pod/logs`.
  pub fn resolved_log_dir(&self) -> PathBuf {
    self.storage.log_dir().clone().unwrap_or_else(|| {
      dir_spec::state_home()
        .map(|p| p.join("pod/logs"))
        .expect("cannot determine state home directory")
    })
  }

  /// Replace the feature flag configuration.
  pub fn set_features(&mut self, features: features::Settings) {
    self.features = features;
  }

  /// Replace the storage path configuration.
  pub fn set_storage(&mut self, storage: storage::Settings) {
    self.storage = storage;
  }
}

/// Initializes the global `Settings` singleton.
///
/// Must be called exactly once before [`Settings::global`] is used.
/// Subsequent calls are silently ignored.
pub fn init_global(settings: Settings) {
  let _ = GLOBAL.set(settings);
}

/// Loads application configuration from `{config_home}/pod/config.toml`,
/// falling back to defaults if the file is absent or cannot be parsed.
pub fn load() -> Result<Settings, Error> {
  let path = dir_spec::config_home()
    .map(|p| p.join("pod/config.toml"))
    .ok_or(Error::ConfigDirNotFound)?;

  Figment::from(Serialized::defaults(Settings::default()))
    .merge(Toml::file(path))
    .extract()
    .map_err(|e| Box::new(e).into())
}

/// Persists the current configuration to `{config_home}/pod/config.toml`.
///
/// Silently no-ops if the config directory cannot be determined or the write
/// fails — config loss is preferable to a crash on save.
pub fn save(settings: &Settings) {
  let Some(path) = dir_spec::config_home().map(|p| p.join("pod/config.toml")) else {
    tracing::warn!("config: could not determine config directory — settings not saved");
    return;
  };
  ensure_config_dir(&path);
  write_config_file(&path, settings);
}

fn ensure_config_dir(path: &std::path::Path) {
  if let Some(parent) = path.parent()
    && let Err(e) = std::fs::create_dir_all(parent)
  {
    tracing::warn!("config: failed to create config directory: {e}");
  }
}

fn write_config_file(path: &std::path::Path, settings: &Settings) {
  match toml::to_string_pretty(settings) {
    Ok(content) => {
      if let Err(e) = std::fs::write(path, content) {
        tracing::warn!("config: failed to write config file: {e}");
      }
    }
    Err(e) => tracing::warn!("config: failed to serialize settings: {e}"),
  }
}
