use std::path::{Path, PathBuf};

use figment::{
  Figment,
  providers::{Format, Serialized, Toml},
};
use getset::{Getters, MutGetters, Setters};
use serde::{Deserialize, Serialize};

const EVE_CLIENT_ID: &str = "d2de5275730e40da8c15149c464b9c39";

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("failed to determine the user's config directory")]
  ConfigDirNotFound,
  #[error(transparent)]
  Load(#[from] Box<figment::Error>),
  #[error("failed to write config: {0}")]
  Write(String),
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Feature {
  AssetTracking,
  CloneMonitoring,
  CombatLog,
  Contacts,
  EveNotifications,
  LocationTracking,
  Mail,
  SkillMonitoring,
  Standings,
  Wallet,
}

impl Feature {
  #[allow(dead_code)]
  pub const ALL: [Feature; 10] = [
    Feature::CloneMonitoring,
    Feature::Contacts,
    Feature::CombatLog,
    Feature::EveNotifications,
    Feature::Standings,
    Feature::LocationTracking,
    Feature::SkillMonitoring,
    Feature::Mail,
    Feature::Wallet,
    Feature::AssetTracking,
  ];
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Getters, PartialEq, Serialize, Setters)]
#[getset(set = "pub")]
pub struct FeatureFlags {
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  asset_tracking: bool,
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  clone_monitoring: bool,
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  combat_log: bool,
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  contacts: bool,
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  eve_notifications: bool,
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  location_tracking: bool,
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  mail: bool,
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  skill_monitoring: bool,
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  standings: bool,
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  wallet: bool,
}

#[allow(dead_code)]
impl FeatureFlags {
  pub fn enabled(&self) -> Vec<Feature> {
    Feature::ALL
      .into_iter()
      .filter(|&feature| self.is_enabled(feature))
      .collect()
  }

  pub fn is_enabled(&self, feature: Feature) -> bool {
    match feature {
      Feature::AssetTracking => self.asset_tracking,
      Feature::CloneMonitoring => self.clone_monitoring,
      Feature::CombatLog => self.combat_log,
      Feature::Contacts => self.contacts,
      Feature::EveNotifications => self.eve_notifications,
      Feature::LocationTracking => self.location_tracking,
      Feature::Mail => self.mail,
      Feature::SkillMonitoring => self.skill_monitoring,
      Feature::Standings => self.standings,
      Feature::Wallet => self.wallet,
    }
  }

  pub fn set_enabled(&mut self, feature: Feature, value: bool) {
    match feature {
      Feature::AssetTracking => self.asset_tracking = value,
      Feature::CloneMonitoring => self.clone_monitoring = value,
      Feature::CombatLog => self.combat_log = value,
      Feature::Contacts => self.contacts = value,
      Feature::EveNotifications => self.eve_notifications = value,
      Feature::LocationTracking => self.location_tracking = value,
      Feature::Mail => self.mail = value,
      Feature::SkillMonitoring => self.skill_monitoring = value,
      Feature::Standings => self.standings = value,
      Feature::Wallet => self.wallet = value,
    }
  }
}

impl Default for FeatureFlags {
  fn default() -> Self {
    Self {
      asset_tracking: true,
      clone_monitoring: true,
      combat_log: true,
      contacts: true,
      eve_notifications: true,
      location_tracking: true,
      mail: true,
      skill_monitoring: true,
      standings: true,
      wallet: true,
    }
  }
}

#[derive(Clone, Debug, Deserialize, Getters, MutGetters, Serialize)]
pub struct Settings {
  #[getset(get = "pub")]
  #[serde(default = "default_eve_client_id")]
  eve_client_id: String,
  #[getset(get = "pub", get_mut = "pub")]
  #[serde(default)]
  features: FeatureFlags,
  #[getset(get = "pub", get_mut = "pub")]
  #[serde(default)]
  storage: StorageConfig,
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      eve_client_id: default_eve_client_id(),
      features: FeatureFlags::default(),
      storage: StorageConfig::default(),
    }
  }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Getters, PartialEq, Serialize, Setters)]
#[getset(set = "pub")]
pub struct StorageConfig {
  #[getset(get = "pub")]
  #[serde(default, skip_serializing_if = "Option::is_none")]
  cache_dir: Option<PathBuf>,
  #[getset(get = "pub")]
  #[serde(default, skip_serializing_if = "Option::is_none")]
  db_dir: Option<PathBuf>,
  #[getset(get = "pub")]
  #[serde(default, skip_serializing_if = "Option::is_none")]
  log_dir: Option<PathBuf>,
  #[getset(get = "pub")]
  #[serde(default)]
  network: bool,
}

impl StorageConfig {
  pub fn resolved_cache_dir(&self) -> PathBuf {
    self.cache_dir.clone().unwrap_or_else(cache_dir)
  }

  pub fn resolved_database_path(&self) -> PathBuf {
    self.resolved_db_dir().join("pod.db")
  }

  pub fn resolved_db_dir(&self) -> PathBuf {
    self.db_dir.clone().unwrap_or_else(data_dir)
  }

  #[allow(dead_code)]
  pub fn resolved_log_dir(&self) -> PathBuf {
    self.log_dir.clone().unwrap_or_else(log_dir)
  }
}

pub fn cache_dir() -> PathBuf {
  dir_spec::cache_home()
    .unwrap_or_else(|| data_dir().join("cache"))
    .join("pod")
}

pub fn config_file_path() -> Option<PathBuf> {
  config_path().ok()
}

fn config_path() -> Result<PathBuf, Error> {
  dir_spec::config_home()
    .map(|dir| dir.join("pod").join("config.toml"))
    .ok_or(Error::ConfigDirNotFound)
}

pub fn data_dir() -> PathBuf {
  dir_spec::data_home().unwrap_or_else(|| PathBuf::from(".")).join("pod")
}

#[allow(dead_code)]
pub fn database_path() -> PathBuf {
  data_dir().join("pod.db")
}

fn default_eve_client_id() -> String {
  EVE_CLIENT_ID.to_owned()
}

fn default_true() -> bool {
  true
}

pub fn load() -> Result<Settings, Error> {
  load_from(&config_path()?)
}

fn load_from(path: &Path) -> Result<Settings, Error> {
  Figment::from(Serialized::defaults(Settings::default()))
    .merge(Toml::file(path))
    .extract()
    .map_err(|error| Error::Load(Box::new(error)))
}

pub fn log_dir() -> PathBuf {
  data_dir().join("logs")
}

/// Locates the directory containing the bundled assets, preferring the dev manifest dir, then a macOS .app bundle's ../Resources, then the executable's own dir.
pub fn resource_dir() -> PathBuf {
  let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  if manifest.join("assets").is_dir() {
    return manifest;
  }

  if let Ok(exe) = std::env::current_exe()
    && let Some(dir) = exe.parent()
  {
    for candidate in [dir.join("../Resources"), dir.to_path_buf()] {
      if candidate.join("assets").is_dir() {
        return candidate;
      }
    }
  }

  manifest
}

pub fn save(settings: &Settings) {
  match config_path() {
    Ok(path) => {
      if let Err(error) = save_to(&path, settings) {
        tracing::warn!(%error, "failed to write config");
      }
    }
    Err(error) => tracing::warn!(%error, "failed to resolve config path"),
  }
}

fn save_to(path: &Path, settings: &Settings) -> Result<(), Error> {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).map_err(|error| Error::Write(error.to_string()))?;
  }
  let content = toml::to_string_pretty(settings).map_err(|error| Error::Write(error.to_string()))?;
  std::fs::write(path, content).map_err(|error| Error::Write(error.to_string()))
}

#[cfg(test)]
mod tests {
  use super::*;

  mod database_path {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_pod_db_under_the_data_dir() {
      let path = database_path();

      assert_eq!(path.file_name().unwrap(), "pod.db");
      assert!(path.starts_with(data_dir()));
    }
  }

  mod feature_flags {
    use super::*;

    #[test]
    fn set_enabled_flips_the_named_capability_only() {
      let mut flags = FeatureFlags::default();

      flags.set_enabled(Feature::Wallet, false);

      assert!(!flags.is_enabled(Feature::Wallet));
      assert!(flags.is_enabled(Feature::Mail), "other capabilities are untouched");
      assert!(!flags.enabled().contains(&Feature::Wallet));
    }

    #[test]
    fn set_enabled_round_trips_every_capability() {
      for feature in Feature::ALL {
        let mut flags = FeatureFlags::default();
        flags.set_enabled(feature, false);
        assert!(!flags.is_enabled(feature), "{feature:?} should be off");
        flags.set_enabled(feature, true);
        assert!(flags.is_enabled(feature), "{feature:?} should be back on");
      }
    }
  }

  mod resolved_paths {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_the_platform_default_database_path_with_no_override() {
      let storage = StorageConfig::default();

      assert_eq!(storage.resolved_database_path(), data_dir().join("pod.db"));
    }

    #[test]
    fn it_uses_the_db_dir_override_for_the_database_path() {
      let mut storage = StorageConfig::default();
      storage.set_db_dir(Some(PathBuf::from("/var/pod/db")));

      assert_eq!(storage.resolved_db_dir(), PathBuf::from("/var/pod/db"));
      assert_eq!(storage.resolved_database_path(), PathBuf::from("/var/pod/db/pod.db"));
    }

    #[test]
    fn it_resolves_the_log_override() {
      let mut storage = StorageConfig::default();
      storage.set_log_dir(Some(PathBuf::from("/var/pod/log")));

      assert_eq!(storage.resolved_log_dir(), PathBuf::from("/var/pod/log"));
    }

    #[test]
    fn it_uses_the_platform_default_cache_dir_with_no_override() {
      let storage = StorageConfig::default();

      assert_eq!(storage.resolved_cache_dir(), cache_dir());
    }

    #[test]
    fn it_resolves_the_cache_override() {
      let mut storage = StorageConfig::default();
      storage.set_cache_dir(Some(PathBuf::from("/var/pod/cache")));

      assert_eq!(storage.resolved_cache_dir(), PathBuf::from("/var/pod/cache"));
    }
  }

  mod load_from {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_defaults_when_the_file_is_absent() {
      let dir = tempfile::tempdir().unwrap();

      let settings = load_from(&dir.path().join("config.toml")).unwrap();

      assert_eq!(settings.eve_client_id(), EVE_CLIENT_ID);
    }

    #[test]
    fn it_defaults_every_feature_flag_to_enabled_when_the_file_is_absent() {
      let dir = tempfile::tempdir().unwrap();

      let settings = load_from(&dir.path().join("config.toml")).unwrap();

      assert_eq!(settings.features().enabled(), Feature::ALL.to_vec());
    }

    #[test]
    fn it_defaults_the_storage_table_when_the_file_is_absent() {
      let dir = tempfile::tempdir().unwrap();

      let settings = load_from(&dir.path().join("config.toml")).unwrap();
      let storage = settings.storage();

      assert!(!storage.network());
      assert_eq!(*storage.db_dir(), None);
      assert_eq!(*storage.log_dir(), None);
      assert_eq!(*storage.cache_dir(), None);
    }

    #[test]
    fn it_extracts_a_legacy_file_with_only_the_client_id() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, r#"eve_client_id = "byo-client-id""#).unwrap();

      let settings = load_from(&path).unwrap();

      assert_eq!(settings.eve_client_id(), "byo-client-id");
      assert_eq!(settings.features(), &FeatureFlags::default());
      assert_eq!(settings.storage(), &StorageConfig::default());
    }

    #[test]
    fn it_reads_overrides_from_the_file() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, r#"eve_client_id = "byo-client-id""#).unwrap();

      let settings = load_from(&path).unwrap();

      assert_eq!(settings.eve_client_id(), "byo-client-id");
    }

    #[test]
    fn it_reads_feature_overrides_and_keeps_unlisted_flags_enabled() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "[features]\nwallet = false\nmail = false\n").unwrap();

      let features = load_from(&path).unwrap().features().to_owned();

      assert!(!features.wallet());
      assert!(!features.mail());
      assert!(features.clone_monitoring());
      assert!(features.is_enabled(Feature::Contacts));
      assert!(!features.is_enabled(Feature::Wallet));
      assert!(!features.enabled().contains(&Feature::Wallet));
    }

    #[test]
    fn it_reads_storage_overrides_per_field() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(
        &path,
        "[storage]\nnetwork = true\ndb_dir = \"/var/pod/db\"\nlog_dir = \"/var/pod/log\"\ncache_dir = \"/var/pod/cache\"\n",
      )
      .unwrap();

      let storage = load_from(&path).unwrap().storage().to_owned();

      assert!(storage.network());
      assert_eq!(*storage.db_dir(), Some(PathBuf::from("/var/pod/db")));
      assert_eq!(*storage.log_dir(), Some(PathBuf::from("/var/pod/log")));
      assert_eq!(*storage.cache_dir(), Some(PathBuf::from("/var/pod/cache")));
    }
  }

  mod save_to {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_roundtrips_through_the_file() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("nested").join("config.toml");
      let settings = Settings {
        eve_client_id: "byo-client-id".to_owned(),
        ..Settings::default()
      };

      save_to(&path, &settings).unwrap();

      assert_eq!(load_from(&path).unwrap().eve_client_id(), "byo-client-id");
    }

    #[test]
    fn it_roundtrips_the_feature_and_storage_tables() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      let mut settings = Settings::default();
      settings.features.wallet = false;
      settings.features.combat_log = false;
      settings.storage.network = true;
      settings.storage.log_dir = Some(PathBuf::from("/tmp/pod-logs"));

      save_to(&path, &settings).unwrap();
      let loaded = load_from(&path).unwrap();

      assert_eq!(loaded.features(), settings.features());
      assert_eq!(loaded.storage(), settings.storage());
      assert!(!loaded.features().wallet());
      assert!(loaded.storage().network());
      assert_eq!(*loaded.storage().log_dir(), Some(PathBuf::from("/tmp/pod-logs")));
    }
  }
}
