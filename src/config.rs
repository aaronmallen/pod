use std::path::{Path, PathBuf};

use figment::{
  Figment,
  providers::{Format, Serialized, Toml},
};
use getset::{Getters, MutGetters, Setters};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::store::fs_kind::{self, FsKind};

const EVE_CLIENT_ID: &str = "d2de5275730e40da8c15149c464b9c39";
const WORKING_COPY_DB_NAME: &str = "pod.db";
const WORKING_COPY_SUBDIR: &str = "db";

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
  #[serde(default, skip_serializing_if = "Option::is_none")]
  machine_id: Option<String>,
  #[getset(get = "pub")]
  #[serde(default)]
  network: bool,
}

impl StorageConfig {
  fn mode_from(network_override: bool, kind: FsKind) -> StorageMode {
    if network_override || kind.is_network() {
      StorageMode::Sync
    } else {
      StorageMode::Direct
    }
  }

  #[allow(dead_code)]
  pub fn machine_id_or_generate(&mut self) -> String {
    if let Some(id) = &self.machine_id {
      return id.clone();
    }

    let id = generate_machine_id();
    self.machine_id = Some(id.clone());
    id
  }

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

  pub fn resolved_working_copy_path(&self) -> PathBuf {
    self
      .resolved_cache_dir()
      .join(WORKING_COPY_SUBDIR)
      .join(WORKING_COPY_DB_NAME)
  }

  pub fn storage_mode(&self) -> StorageMode {
    self.storage_mode_with(fs_kind::detect)
  }

  fn storage_mode_with(&self, detect: impl Fn(&Path) -> FsKind) -> StorageMode {
    Self::mode_from(self.network, detect(&self.resolved_db_dir()))
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageMode {
  Direct,
  Sync,
}

impl StorageMode {
  #[allow(dead_code)]
  pub fn is_sync(self) -> bool {
    matches!(self, StorageMode::Sync)
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
  resolve_data_dir(dir_spec::data_home(), std::env::temp_dir())
}

fn resolve_data_dir(data_home: Option<PathBuf>, fallback_root: PathBuf) -> PathBuf {
  data_home.unwrap_or(fallback_root).join("pod")
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

#[allow(dead_code)]
fn generate_machine_id() -> String {
  let mut bytes = [0u8; 16];
  rand::rng().fill_bytes(&mut bytes);

  let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
  format!(
    "{}-{}-{}-{}-{}",
    &hex[0..8],
    &hex[8..12],
    &hex[12..16],
    &hex[16..20],
    &hex[20..32]
  )
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
  resolve_log_dir(dir_spec::state_home(), std::env::temp_dir())
}

fn resolve_log_dir(state_home: Option<PathBuf>, fallback_root: PathBuf) -> PathBuf {
  state_home.unwrap_or(fallback_root).join("pod").join("logs")
}

/// Locates the directory containing the bundled assets, preferring the dev manifest dir, then the per-platform packaged candidates (see [`select_resource_dir`]).
pub fn resource_dir() -> PathBuf {
  let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  if manifest.join("assets").is_dir() {
    return manifest;
  }

  if let Ok(exe) = std::env::current_exe()
    && let Some(dir) = exe.parent()
  {
    let binary_name = exe.file_stem().and_then(|stem| stem.to_str());
    if let Some(candidate) = select_resource_dir(dir, binary_name, |path| path.join("assets").is_dir()) {
      return candidate;
    }
  }

  manifest
}

/// Probes the packaged-layout candidates in precedence order — `../Resources` (macOS .app), then
/// `../lib/<binary_name>` (Linux FHS deb/pacman/AppImage, where the binary sits in `usr/bin`), then
/// the executable's own dir (Windows/portable) — returning the first whose `assets` dir is present.
fn select_resource_dir(
  exe_dir: &Path,
  binary_name: Option<&str>,
  has_assets: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
  let mut candidates = vec![exe_dir.join("../Resources")];
  if let Some(name) = binary_name {
    candidates.push(exe_dir.join("../lib").join(name));
  }
  candidates.push(exe_dir.to_path_buf());

  candidates.into_iter().find(|candidate| has_assets(candidate))
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

  mod resolve_data_dir {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_the_data_home_when_present() {
      let resolved = resolve_data_dir(Some(PathBuf::from("/home/me/.local/share")), PathBuf::from("/tmp"));

      assert_eq!(resolved, PathBuf::from("/home/me/.local/share/pod"));
    }

    #[test]
    fn it_falls_back_to_the_given_root_when_data_home_is_missing() {
      let resolved = resolve_data_dir(None, PathBuf::from("/var/tmp"));

      assert_eq!(resolved, PathBuf::from("/var/tmp/pod"));
    }

    #[test]
    fn its_fallback_is_absolute_and_not_relative_to_the_current_directory() {
      let resolved = resolve_data_dir(None, std::env::temp_dir());

      assert!(
        resolved.is_absolute(),
        "the fallback must not depend on the working directory"
      );
      assert_ne!(resolved, PathBuf::from("./pod"));
    }
  }

  mod resolve_log_dir {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_uses_the_state_home_when_present() {
      let resolved = resolve_log_dir(Some(PathBuf::from("/home/me/.local/state")), PathBuf::from("/tmp"));

      assert_eq!(resolved, PathBuf::from("/home/me/.local/state/pod/logs"));
    }

    #[test]
    fn it_falls_back_to_the_given_root_when_state_home_is_missing() {
      let resolved = resolve_log_dir(None, PathBuf::from("/var/tmp"));

      assert_eq!(resolved, PathBuf::from("/var/tmp/pod/logs"));
    }

    #[test]
    fn its_fallback_is_absolute_and_not_relative_to_the_current_directory() {
      let resolved = resolve_log_dir(None, std::env::temp_dir());

      assert!(
        resolved.is_absolute(),
        "the fallback must not depend on the working directory"
      );
      assert_ne!(resolved, PathBuf::from("./pod/logs"));
    }
  }

  mod select_resource_dir {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_selects_the_macos_resources_bundle() {
      let exe_dir = PathBuf::from("/Applications/pod.app/Contents/MacOS");
      let resources = exe_dir.join("../Resources");

      let resolved = select_resource_dir(&exe_dir, Some("pod"), |path| *path == resources);

      assert_eq!(resolved, Some(resources));
    }

    #[test]
    fn it_selects_the_linux_lib_dir_for_the_fhs_layout() {
      let exe_dir = PathBuf::from("/usr/bin");
      let lib_dir = exe_dir.join("../lib").join("pod");

      let resolved = select_resource_dir(&exe_dir, Some("pod"), |path| *path == lib_dir);

      assert_eq!(resolved, Some(PathBuf::from("/usr/bin/../lib/pod")));
    }

    #[test]
    fn it_selects_the_exe_dir_for_the_windows_layout() {
      let exe_dir = PathBuf::from("C:/Program Files/pod");

      let resolved = select_resource_dir(&exe_dir, Some("pod"), |path| *path == exe_dir);

      assert_eq!(resolved, Some(exe_dir));
    }

    #[test]
    fn it_prefers_the_resources_bundle_over_the_exe_dir() {
      let exe_dir = PathBuf::from("/opt/pod");
      let resources = exe_dir.join("../Resources");

      let resolved = select_resource_dir(&exe_dir, Some("pod"), |path| *path == resources || *path == exe_dir);

      assert_eq!(resolved, Some(resources));
    }

    #[test]
    fn it_skips_the_linux_candidate_when_the_binary_name_is_unknown() {
      let exe_dir = PathBuf::from("/usr/bin");
      let lib_dir = exe_dir.join("../lib").join("pod");

      let resolved = select_resource_dir(&exe_dir, None, |path| *path == lib_dir);

      assert_eq!(resolved, None);
    }

    #[test]
    fn it_returns_none_when_no_candidate_holds_the_assets() {
      let exe_dir = PathBuf::from("/usr/bin");

      let resolved = select_resource_dir(&exe_dir, Some("pod"), |_| false);

      assert_eq!(resolved, None);
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
    fn it_uses_the_state_home_default_log_dir_with_no_override() {
      let storage = StorageConfig::default();

      assert_eq!(storage.resolved_log_dir(), log_dir());
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

  mod storage_mode {
    use pretty_assertions::assert_eq;

    use super::*;

    fn always(kind: FsKind) -> impl Fn(&Path) -> FsKind {
      move |_| kind
    }

    #[test]
    fn it_is_direct_for_a_local_path_with_no_override() {
      let storage = StorageConfig::default();

      assert_eq!(storage.storage_mode_with(always(FsKind::Local)), StorageMode::Direct);
    }

    #[test]
    fn it_is_sync_when_detection_reports_a_network_path() {
      let storage = StorageConfig::default();

      assert_eq!(storage.storage_mode_with(always(FsKind::Network)), StorageMode::Sync);
    }

    #[test]
    fn it_forces_sync_when_the_manual_flag_is_set_even_when_detection_says_local() {
      let mut storage = StorageConfig::default();
      storage.set_network(true);

      assert_eq!(storage.storage_mode_with(always(FsKind::Local)), StorageMode::Sync);
    }

    #[test]
    fn it_detects_against_the_resolved_db_dir() {
      let mut storage = StorageConfig::default();
      storage.set_db_dir(Some(PathBuf::from("/mnt/nas/pod")));
      let seen = std::cell::RefCell::new(None);

      let mode = storage.storage_mode_with(|path| {
        *seen.borrow_mut() = Some(path.to_path_buf());
        FsKind::Local
      });

      assert_eq!(mode, StorageMode::Direct);
      assert_eq!(seen.into_inner(), Some(PathBuf::from("/mnt/nas/pod")));
    }
  }

  mod resolved_working_copy_path {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_lives_under_the_evictable_cache_dir() {
      let mut storage = StorageConfig::default();
      storage.set_cache_dir(Some(PathBuf::from("/var/pod/cache")));

      let path = storage.resolved_working_copy_path();

      assert!(path.starts_with(storage.resolved_cache_dir()));
      assert_eq!(path.file_name().unwrap(), "pod.db");
    }

    #[test]
    fn it_is_distinct_from_the_shared_db_path() {
      let mut storage = StorageConfig::default();
      storage.set_cache_dir(Some(PathBuf::from("/var/pod/cache")));
      storage.set_db_dir(Some(PathBuf::from("/mnt/nas/pod")));

      assert_ne!(storage.resolved_working_copy_path(), storage.resolved_database_path());
    }
  }

  mod machine_id_or_generate {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_generates_and_persists_a_machine_id_once() {
      let mut storage = StorageConfig::default();
      assert_eq!(*storage.machine_id(), None);

      let first = storage.machine_id_or_generate();

      assert!(!first.is_empty());
      assert_eq!(*storage.machine_id(), Some(first.clone()));
    }

    #[test]
    fn it_returns_the_same_id_on_subsequent_calls() {
      let mut storage = StorageConfig::default();

      let first = storage.machine_id_or_generate();
      let second = storage.machine_id_or_generate();

      assert_eq!(first, second);
    }

    #[test]
    fn it_round_trips_a_generated_id_through_the_file() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      let mut settings = Settings::default();
      let id = settings.storage_mut().machine_id_or_generate();

      save_to(&path, &settings).unwrap();
      let loaded = load_from(&path).unwrap();

      assert_eq!(*loaded.storage().machine_id(), Some(id));
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
      assert_eq!(*storage.machine_id(), None);
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

  mod serialization {
    use super::*;

    #[test]
    fn a_default_storage_config_serializes_without_any_dir_keys() {
      let toml = toml::to_string_pretty(&StorageConfig::default()).unwrap();

      assert!(
        !toml.contains("cache_dir"),
        "resolved cache_dir must not leak to disk: {toml}"
      );
      assert!(
        !toml.contains("db_dir"),
        "resolved db_dir must not leak to disk: {toml}"
      );
      assert!(
        !toml.contains("log_dir"),
        "resolved log_dir must not leak to disk: {toml}"
      );
      assert!(
        !toml.contains("machine_id"),
        "an unset machine_id must not leak to disk: {toml}"
      );
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
