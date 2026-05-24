//! Application configuration loading and persistence.

pub mod features;

use figment::{
  Figment,
  providers::{Format, Serialized, Toml},
};
use getset::Getters;
use serde::{Deserialize, Serialize};

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
}

impl Settings {
  /// Replace the feature flag configuration.
  pub fn set_features(&mut self, features: features::Settings) {
    self.features = features;
  }
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
  if let Some(parent) = path.parent()
    && let Err(e) = std::fs::create_dir_all(parent)
  {
    tracing::warn!("config: failed to create config directory: {e}");
  }
  match toml::to_string_pretty(settings) {
    Ok(content) => {
      if let Err(e) = std::fs::write(&path, content) {
        tracing::warn!("config: failed to write config file: {e}");
      }
    }
    Err(e) => tracing::warn!("config: failed to serialize settings: {e}"),
  }
}
