use std::path::{Path, PathBuf};

use figment::{
  Figment,
  providers::{Format, Serialized, Toml},
};
use getset::{CopyGetters, Getters, MutGetters, Setters};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{
  i18n::Language,
  store::fs_kind::{self, FsKind},
  ui::components::rail::Destination,
};

const EVE_CLIENT_ID: &str = "d2de5275730e40da8c15149c464b9c39";
const WORKING_COPY_DB_NAME: &str = "pod.db";
const WORKING_COPY_SUBDIR: &str = "db";

#[derive(Clone, Copy, CopyGetters, Debug, Deserialize, Eq, Getters, PartialEq, Serialize, Setters)]
#[getset(set = "pub")]
pub struct AccessibilityConfig {
  #[getset(get = "pub")]
  #[serde(default, skip_serializing_if = "is_not_high_contrast")]
  high_contrast: bool,
  #[getset(get_copy = "pub")]
  #[serde(default, skip_serializing_if = "is_default_language")]
  language: Language,
  #[getset(get = "pub")]
  #[serde(default = "default_scale_100", skip_serializing_if = "is_default_scale")]
  scale: u8,
}

impl AccessibilityConfig {
  fn is_default(&self) -> bool {
    *self == AccessibilityConfig::default()
  }
}

impl Default for AccessibilityConfig {
  fn default() -> Self {
    Self {
      high_contrast: false,
      language: Language::EnUs,
      scale: default_scale_100(),
    }
  }
}

#[derive(Clone, Copy, CopyGetters, Debug, Deserialize, Eq, PartialEq, Serialize, Setters)]
#[getset(set = "pub")]
pub struct CalendarTweaks {
  #[getset(get_copy = "pub")]
  #[serde(default = "default_true")]
  color_by_pilot: bool,
  #[getset(get_copy = "pub")]
  #[serde(default = "default_calendar_density")]
  density: CalendarDensity,
  #[getset(get_copy = "pub")]
  #[serde(default = "default_true")]
  local_time: bool,
  #[getset(get_copy = "pub")]
  #[serde(default = "default_true")]
  month_chips: bool,
  #[getset(get_copy = "pub")]
  #[serde(default = "default_true")]
  pod_overlays: bool,
  #[getset(get_copy = "pub")]
  #[serde(default = "default_true")]
  show_weekends: bool,
  #[getset(get_copy = "pub")]
  #[serde(default = "default_true")]
  week_hours: bool,
  #[getset(get_copy = "pub")]
  #[serde(default = "default_calendar_week_start")]
  week_start: CalendarWeekStart,
}

impl Default for CalendarTweaks {
  fn default() -> Self {
    Self {
      color_by_pilot: true,
      density: CalendarDensity::default(),
      local_time: true,
      month_chips: true,
      pod_overlays: true,
      show_weekends: true,
      week_hours: true,
      week_start: CalendarWeekStart::Sunday,
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CalendarDensity {
  #[default]
  Comfortable,
  Compact,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CalendarWeekStart {
  #[default]
  Monday,
  Sunday,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CascadeMode {
  #[default]
  Flyout,
  None,
  SubRail,
}

impl CascadeMode {
  pub const ALL: [CascadeMode; 3] = [CascadeMode::Flyout, CascadeMode::SubRail, CascadeMode::None];

  pub fn label(self) -> String {
    match self {
      CascadeMode::Flyout => t!("config.cascade_mode.flyout"),
      CascadeMode::None => t!("config.cascade_mode.none"),
      CascadeMode::SubRail => t!("config.cascade_mode.sub_rail"),
    }
    .into_owned()
  }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("failed to determine the user's config directory")]
  ConfigDirNotFound,
  #[error(transparent)]
  Load(#[from] Box<figment::Error>),
  #[error("failed to write config: {0}")]
  Write(String),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Feature {
  AssetTracking,
  Calendar,
  CloneMonitoring,
  CombatLog,
  Contacts,
  EveNotifications,
  Industry,
  LocationTracking,
  Mail,
  SkillMonitoring,
  Standings,
  Wallet,
}

impl Feature {
  pub const ALL: [Feature; 12] = [
    Feature::CloneMonitoring,
    Feature::Contacts,
    Feature::CombatLog,
    Feature::EveNotifications,
    Feature::Standings,
    Feature::LocationTracking,
    Feature::SkillMonitoring,
    Feature::Industry,
    Feature::Mail,
    Feature::Calendar,
    Feature::Wallet,
    Feature::AssetTracking,
  ];

  /// The legacy flat TOML key for this group, used to migrate pre-sub-feature configs.
  pub fn legacy_key(self) -> &'static str {
    match self {
      Feature::AssetTracking => "asset_tracking",
      Feature::Calendar => "calendar",
      Feature::CloneMonitoring => "clone_monitoring",
      Feature::CombatLog => "combat_log",
      Feature::Contacts => "contacts",
      Feature::EveNotifications => "eve_notifications",
      Feature::Industry => "industry",
      Feature::LocationTracking => "location_tracking",
      Feature::Mail => "mail",
      Feature::SkillMonitoring => "skill_monitoring",
      Feature::Standings => "standings",
      Feature::Wallet => "wallet",
    }
  }

  pub fn noun(self) -> &'static str {
    match self {
      Feature::AssetTracking => "Asset",
      Feature::Calendar => "Calendar event",
      Feature::CloneMonitoring => "Clone",
      Feature::CombatLog => "Kill log",
      Feature::Contacts => "Contact",
      Feature::EveNotifications => "Notification",
      Feature::Industry => "Industry job",
      Feature::LocationTracking => "Location",
      Feature::Mail => "Mail",
      Feature::SkillMonitoring => "Skill",
      Feature::Standings => "Standing",
      Feature::Wallet => "Wallet",
    }
  }

  pub fn sub_features(self) -> &'static [SubFeature] {
    match self {
      Feature::AssetTracking => &[
        SubFeature::Inventory,
        SubFeature::Abyssals,
        SubFeature::Stockpiles,
        SubFeature::Values,
        SubFeature::Tracker,
      ],
      Feature::Calendar => &[SubFeature::Calendar],
      Feature::CloneMonitoring => &[SubFeature::CloneMonitoring],
      Feature::CombatLog => &[SubFeature::KillLog],
      Feature::Contacts => &[SubFeature::Contacts],
      Feature::EveNotifications => &[SubFeature::Notifications],
      Feature::Industry => &[
        SubFeature::JobMonitoring,
        SubFeature::Blueprints,
        SubFeature::Planner,
        SubFeature::Extractions,
      ],
      Feature::LocationTracking => &[SubFeature::LocationTracking],
      Feature::Mail => &[SubFeature::Mail],
      Feature::SkillMonitoring => &[SubFeature::SkillQueue],
      Feature::Standings => &[SubFeature::Standings],
      Feature::Wallet => &[
        SubFeature::Wallets,
        SubFeature::Transactions,
        SubFeature::Contracts,
        SubFeature::Journal,
        SubFeature::Budget,
      ],
    }
  }
}

/// One independently-toggleable capability nested under a top-level [`Feature`] group. The granular
/// level of the two-level feature model; group enablement rolls up as "any child enabled".
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SubFeature {
  Abyssals,
  Blueprints,
  Budget,
  Calendar,
  CloneMonitoring,
  Contacts,
  Contracts,
  Extractions,
  Inventory,
  JobMonitoring,
  Journal,
  KillLog,
  LocationTracking,
  Mail,
  Notifications,
  Planner,
  SkillQueue,
  Stockpiles,
  Standings,
  Tracker,
  Transactions,
  Values,
  Wallets,
}

impl SubFeature {
  pub const ALL: [SubFeature; 23] = [
    SubFeature::Abyssals,
    SubFeature::Blueprints,
    SubFeature::Budget,
    SubFeature::Calendar,
    SubFeature::CloneMonitoring,
    SubFeature::Contacts,
    SubFeature::Contracts,
    SubFeature::Extractions,
    SubFeature::Inventory,
    SubFeature::JobMonitoring,
    SubFeature::Journal,
    SubFeature::KillLog,
    SubFeature::LocationTracking,
    SubFeature::Mail,
    SubFeature::Notifications,
    SubFeature::Planner,
    SubFeature::SkillQueue,
    SubFeature::Stockpiles,
    SubFeature::Standings,
    SubFeature::Tracker,
    SubFeature::Transactions,
    SubFeature::Values,
    SubFeature::Wallets,
  ];

  // Sub-feature -> group roll-up consumed by sibling tasks B/C; today only the tests use it.
  #[allow(dead_code)]
  pub fn group(self) -> Feature {
    match self {
      SubFeature::Abyssals
      | SubFeature::Inventory
      | SubFeature::Stockpiles
      | SubFeature::Tracker
      | SubFeature::Values => Feature::AssetTracking,
      SubFeature::Calendar => Feature::Calendar,
      SubFeature::CloneMonitoring => Feature::CloneMonitoring,
      SubFeature::KillLog => Feature::CombatLog,
      SubFeature::Contacts => Feature::Contacts,
      SubFeature::Notifications => Feature::EveNotifications,
      SubFeature::Blueprints | SubFeature::Extractions | SubFeature::JobMonitoring | SubFeature::Planner => {
        Feature::Industry
      }
      SubFeature::LocationTracking => Feature::LocationTracking,
      SubFeature::Mail => Feature::Mail,
      SubFeature::SkillQueue => Feature::SkillMonitoring,
      SubFeature::Standings => Feature::Standings,
      SubFeature::Budget
      | SubFeature::Contracts
      | SubFeature::Journal
      | SubFeature::Transactions
      | SubFeature::Wallets => Feature::Wallet,
    }
  }

  /// The TOML key for this sub-feature within its group's nested table.
  pub fn key(self) -> &'static str {
    match self {
      SubFeature::Abyssals => "abyssals",
      SubFeature::Blueprints => "blueprints",
      SubFeature::Budget => "budget",
      SubFeature::Calendar => "calendar",
      SubFeature::CloneMonitoring => "clone_monitoring",
      SubFeature::Contacts => "contacts",
      SubFeature::Contracts => "contracts",
      SubFeature::Extractions => "extractions",
      SubFeature::Inventory => "inventory",
      SubFeature::JobMonitoring => "job_monitoring",
      SubFeature::Journal => "journal",
      SubFeature::KillLog => "kill_log",
      SubFeature::LocationTracking => "location_tracking",
      SubFeature::Mail => "mail",
      SubFeature::Notifications => "notifications",
      SubFeature::Planner => "planner",
      SubFeature::SkillQueue => "skill_queue",
      SubFeature::Stockpiles => "stockpiles",
      SubFeature::Standings => "standings",
      SubFeature::Tracker => "tracker",
      SubFeature::Transactions => "transactions",
      SubFeature::Values => "values",
      SubFeature::Wallets => "wallets",
    }
  }
}

/// Per-sub-feature enablement, the persisted form of the two-level feature model.
///
/// Loads tolerantly: a legacy flat config (`wallet = false`) cascades the group's value onto every
/// child, while the new nested config (`[features.wallet] budget = false`) is read per sub-feature.
/// Any sub-feature absent from the file defaults to enabled. Always re-serializes in the nested form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeatureFlags {
  enabled: [bool; SubFeature::ALL.len()],
}

impl FeatureFlags {
  pub fn enabled(&self) -> Vec<Feature> {
    Feature::ALL
      .into_iter()
      .filter(|&feature| self.is_enabled(feature))
      .collect()
  }

  pub fn enabled_sub_features(&self) -> Vec<SubFeature> {
    SubFeature::ALL
      .into_iter()
      .filter(|&sub| self.is_sub_enabled(sub))
      .collect()
  }

  // The per-group roll-up's scope/shell consumers land in sibling tasks B/C, so today this is
  // exercised only by this module's tests.
  #[allow(dead_code)]
  pub fn enabled_sub_features_of(&self, feature: Feature) -> Vec<SubFeature> {
    feature
      .sub_features()
      .iter()
      .copied()
      .filter(|&sub| self.is_sub_enabled(sub))
      .collect()
  }

  pub fn is_enabled(&self, feature: Feature) -> bool {
    feature.sub_features().iter().any(|&sub| self.is_sub_enabled(sub))
  }

  pub fn is_sub_enabled(&self, sub: SubFeature) -> bool {
    self.enabled[Self::index_of(sub)]
  }

  pub fn set_enabled(&mut self, feature: Feature, value: bool) {
    for &sub in feature.sub_features() {
      self.set_sub_enabled(sub, value);
    }
  }

  pub fn set_sub_enabled(&mut self, sub: SubFeature, value: bool) {
    self.enabled[Self::index_of(sub)] = value;
    self.enforce_couplings();
  }

  /// Budget has no ESI scope of its own; it derives entirely from Journal and Transactions activity.
  /// It can only be on while at least one of those is on, so a toggle that leaves both off forces
  /// Budget off too (this also makes enabling Budget a no-op while both are disabled).
  fn enforce_couplings(&mut self) {
    let derives_budget =
      self.enabled[Self::index_of(SubFeature::Journal)] || self.enabled[Self::index_of(SubFeature::Transactions)];
    if !derives_budget {
      self.enabled[Self::index_of(SubFeature::Budget)] = false;
    }
  }

  fn index_of(sub: SubFeature) -> usize {
    SubFeature::ALL
      .iter()
      .position(|&candidate| candidate == sub)
      .expect("every SubFeature is listed in SubFeature::ALL")
  }
}

impl Default for FeatureFlags {
  fn default() -> Self {
    Self {
      enabled: [true; SubFeature::ALL.len()],
    }
  }
}

impl<'de> Deserialize<'de> for FeatureFlags {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    use std::collections::BTreeMap;

    use serde::de::Error;

    // A value under a group key is either a legacy flat bool (`wallet = false`) or a new nested
    // table (`[features.wallet] budget = false`). Accept either to migrate transparently.
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum GroupEntry {
      Flat(bool),
      Nested(BTreeMap<String, bool>),
    }

    let raw = BTreeMap::<String, GroupEntry>::deserialize(deserializer)?;
    let mut flags = FeatureFlags::default();

    for feature in Feature::ALL {
      let Some(entry) = raw.get(feature.legacy_key()) else {
        continue;
      };
      match entry {
        GroupEntry::Flat(value) => flags.set_enabled(feature, *value),
        GroupEntry::Nested(children) => {
          for &sub in feature.sub_features() {
            if let Some(value) = children.get(sub.key()) {
              flags.set_sub_enabled(sub, *value);
            }
          }
        }
      }
    }

    for entry in raw.keys() {
      if !Feature::ALL.iter().any(|feature| feature.legacy_key() == entry) {
        return Err(D::Error::custom(format!("unknown feature group `{entry}`")));
      }
    }

    Ok(flags)
  }
}

impl Serialize for FeatureFlags {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    use serde::ser::SerializeMap;

    let mut map = serializer.serialize_map(Some(Feature::ALL.len()))?;
    for feature in Feature::ALL {
      let children: std::collections::BTreeMap<&'static str, bool> = feature
        .sub_features()
        .iter()
        .map(|&sub| (sub.key(), self.is_sub_enabled(sub)))
        .collect();
      map.serialize_entry(feature.legacy_key(), &children)?;
    }
    map.end()
  }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Getters, PartialEq, Serialize, Setters)]
#[getset(set = "pub")]
pub struct IndustryConfig {
  #[getset(get = "pub")]
  #[serde(default, skip_serializing_if = "Option::is_none")]
  manufacturing: Option<i64>,
  #[getset(get = "pub")]
  #[serde(default, skip_serializing_if = "Option::is_none")]
  reactions: Option<i64>,
}

impl IndustryConfig {
  fn is_default(&self) -> bool {
    *self == IndustryConfig::default()
  }
}

#[derive(Clone, Debug, Deserialize, Getters, MutGetters, Serialize, Setters)]
pub struct Settings {
  #[getset(get = "pub", get_mut = "pub")]
  #[serde(default, skip_serializing_if = "AccessibilityConfig::is_default")]
  accessibility: AccessibilityConfig,
  #[getset(get = "pub")]
  #[serde(default = "default_eve_client_id")]
  eve_client_id: String,
  #[getset(get = "pub", get_mut = "pub")]
  #[serde(default)]
  features: FeatureFlags,
  #[getset(get = "pub", get_mut = "pub")]
  #[serde(default, skip_serializing_if = "IndustryConfig::is_default")]
  industry: IndustryConfig,
  #[getset(get = "pub", get_mut = "pub")]
  #[serde(default, skip_serializing_if = "McpConfig::is_default")]
  mcp: McpConfig,
  #[getset(get = "pub", set = "pub")]
  #[serde(default = "default_reprocessing_yield")]
  reprocessing_yield: f64,
  #[getset(get = "pub", get_mut = "pub")]
  #[serde(default)]
  storage: StorageConfig,
  #[getset(get = "pub", get_mut = "pub")]
  #[serde(default, skip_serializing_if = "TelemetryConfig::is_default")]
  telemetry: TelemetryConfig,
  #[getset(get = "pub", get_mut = "pub")]
  #[serde(default, skip_serializing_if = "UiConfig::is_default")]
  ui: UiConfig,
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      accessibility: AccessibilityConfig::default(),
      eve_client_id: default_eve_client_id(),
      features: FeatureFlags::default(),
      industry: IndustryConfig::default(),
      mcp: McpConfig::default(),
      reprocessing_yield: default_reprocessing_yield(),
      storage: StorageConfig::default(),
      telemetry: TelemetryConfig::default(),
      ui: UiConfig::default(),
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
  Normal,
  #[default]
  Quiet,
  Verbose,
}

impl LogLevel {
  pub const ALL: [LogLevel; 3] = [LogLevel::Quiet, LogLevel::Normal, LogLevel::Verbose];

  pub fn label(self) -> String {
    match self {
      LogLevel::Normal => t!("config.log_level.normal"),
      LogLevel::Quiet => t!("config.log_level.quiet"),
      LogLevel::Verbose => t!("config.log_level.verbose"),
    }
    .into_owned()
  }

  fn is_default(&self) -> bool {
    *self == LogLevel::default()
  }
}

/// Configuration for the embedded MCP server an external agent connects to over localhost.
///
/// Off by default: enabling it opens an authenticated automation surface, so `enabled` must be an
/// explicit opt-in. The `token` is the bearer secret every request must present; an empty token is
/// auto-generated on first load by [`McpConfig::token_or_generate`].
#[derive(Clone, Debug, Deserialize, Eq, Getters, PartialEq, Serialize, Setters)]
#[getset(set = "pub")]
pub struct McpConfig {
  #[getset(get = "pub")]
  #[serde(default)]
  enabled: bool,
  #[getset(get = "pub")]
  #[serde(default)]
  perms: McpPerms,
  #[getset(get = "pub")]
  #[serde(default = "default_mcp_port")]
  port: u16,
  #[getset(get = "pub")]
  #[serde(default)]
  token: String,
}

impl McpConfig {
  fn is_default(&self) -> bool {
    *self == McpConfig::default()
  }

  /// Returns the configured bearer token, generating and persisting one in-place when it is empty.
  ///
  /// Called on load so a config that has never set a token (or had it cleared) still presents a
  /// usable secret the moment the server is enabled.
  pub fn token_or_generate(&mut self) -> String {
    if self.token.is_empty() {
      self.token = gen_token();
    }
    self.token.clone()
  }
}

impl Default for McpConfig {
  fn default() -> Self {
    Self {
      enabled: false,
      perms: McpPerms::default(),
      port: default_mcp_port(),
      token: String::new(),
    }
  }
}

/// Opt-out controls for anonymous telemetry, all streams on by default.
///
/// `enabled` is the master switch; the four per-stream toggles (`usage`, `performance`, `crashes`,
/// `environment`) let a user keep telemetry on while silencing an individual stream. Every flag
/// defaults to true, so a default install writes no `[telemetry]` block and opting out of anything
/// persists the explicit `false`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Getters, PartialEq, Serialize, Setters)]
#[getset(set = "pub")]
pub struct TelemetryConfig {
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  enabled: bool,
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  usage: bool,
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  performance: bool,
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  crashes: bool,
  #[getset(get = "pub")]
  #[serde(default = "default_true")]
  environment: bool,
}

impl TelemetryConfig {
  fn is_default(&self) -> bool {
    *self == TelemetryConfig::default()
  }
}

impl Default for TelemetryConfig {
  fn default() -> Self {
    Self {
      enabled: true,
      usage: true,
      performance: true,
      crashes: true,
      environment: true,
    }
  }
}

/// The five-flag trust surface gating what an MCP agent may do. Reads and local writes are on by
/// default; the three mail/label mutations stay off so the riskiest actions are an explicit opt-in.
#[derive(Clone, Copy, CopyGetters, Debug, Deserialize, Eq, PartialEq, Serialize, Setters)]
#[getset(set = "pub")]
pub struct McpPerms {
  #[getset(get_copy = "pub")]
  #[serde(default)]
  delete_mail: bool,
  #[getset(get_copy = "pub")]
  #[serde(default = "default_true")]
  local_write: bool,
  #[getset(get_copy = "pub")]
  #[serde(default)]
  manage_labels: bool,
  #[getset(get_copy = "pub")]
  #[serde(default = "default_true")]
  read: bool,
  #[getset(get_copy = "pub")]
  #[serde(default)]
  send_mail: bool,
}

impl Default for McpPerms {
  fn default() -> Self {
    Self {
      delete_mail: false,
      local_write: true,
      manage_labels: false,
      read: true,
      send_mail: false,
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NavLocation {
  #[default]
  Left,
  Right,
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
  #[serde(default, skip_serializing_if = "LogLevel::is_default")]
  log_level: LogLevel,
  #[getset(get = "pub")]
  #[serde(default, skip_serializing_if = "Option::is_none")]
  machine_id: Option<String>,
  #[getset(get = "pub")]
  #[serde(default)]
  network: bool,
  /// Internal-only override: never persisted or user-settable so the live working copy can't be
  /// redirected onto a network FS.
  #[serde(skip)]
  working_copy_dir: Option<PathBuf>,
}

impl StorageConfig {
  fn mode_from(network_override: bool) -> StorageMode {
    if network_override {
      StorageMode::Sync
    } else {
      StorageMode::Direct
    }
  }

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

  pub fn resolved_log_dir(&self) -> PathBuf {
    self.log_dir.clone().unwrap_or_else(log_dir)
  }

  pub fn resolved_working_copy_path(&self) -> PathBuf {
    self
      .resolved_working_copy_dir(fs_kind::detect)
      .join(WORKING_COPY_DB_NAME)
  }

  fn resolved_working_copy_dir(&self, detect: impl Fn(&Path) -> FsKind) -> PathBuf {
    let base = self.working_copy_dir.clone().unwrap_or_else(default_working_copy_dir);
    // The working copy must stay local; redirect to a temp-dir fallback if the base is on a network FS.
    if detect(&base).is_network() {
      return local_working_copy_fallback();
    }
    base
  }

  pub fn storage_mode(&self) -> StorageMode {
    Self::mode_from(self.network)
  }

  /// Advisory for a UI hint only: reports a network db_dir while sync is off. Never changes the mode.
  pub fn suggests_network_sync(&self) -> bool {
    self.suggests_network_sync_with(fs_kind::detect)
  }

  fn suggests_network_sync_with(&self, detect: impl Fn(&Path) -> FsKind) -> bool {
    !self.network && detect(&self.resolved_db_dir()).is_network()
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageMode {
  Direct,
  Sync,
}

#[derive(Clone, Debug, Deserialize, Eq, Getters, MutGetters, PartialEq, Serialize, Setters)]
#[getset(set = "pub")]
pub struct UiConfig {
  #[getset(get = "pub")]
  #[serde(default, deserialize_with = "deserialize_cascade_mode")]
  cascade_mode: CascadeMode,
  #[getset(get = "pub")]
  #[serde(default)]
  nav_location: NavLocation,
  #[getset(get = "pub", get_mut = "pub")]
  #[serde(default = "default_rail_order", deserialize_with = "deserialize_rail_order")]
  rail_order: Vec<Destination>,
}

impl UiConfig {
  fn is_default(&self) -> bool {
    *self == UiConfig::default()
  }

  pub fn sanitize(&mut self) {
    let mut seen = Vec::with_capacity(Destination::REORDERABLE.len());

    for &destination in &self.rail_order {
      if Destination::REORDERABLE.contains(&destination) && !seen.contains(&destination) {
        seen.push(destination);
      }
    }

    for &destination in &Destination::REORDERABLE {
      if !seen.contains(&destination) {
        seen.push(destination);
      }
    }

    self.rail_order = seen;
  }
}

impl Default for UiConfig {
  fn default() -> Self {
    Self {
      cascade_mode: CascadeMode::default(),
      nav_location: NavLocation::default(),
      rail_order: default_rail_order(),
    }
  }
}

pub fn cache_dir() -> PathBuf {
  dir_spec::cache_home()
    .unwrap_or_else(|| data_dir().join("cache"))
    .join("pod")
}

pub fn config_exists() -> bool {
  config_path().is_ok_and(|path| config_exists_at(&path))
}

fn config_exists_at(path: &Path) -> bool {
  path.is_file()
}

pub fn database_exists(settings: &Settings) -> bool {
  let storage = settings.storage();
  database_exists_at(&storage.resolved_database_path(), &storage.resolved_working_copy_path())
}

fn database_exists_at(canonical: &Path, working_copy: &Path) -> bool {
  canonical.is_file() || working_copy.is_file()
}

pub fn should_run_wizard(settings: &Settings) -> bool {
  is_first_run(config_exists(), database_exists(settings))
}

fn is_first_run(config_present: bool, database_present: bool) -> bool {
  !config_present && !database_present
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

fn default_working_copy_dir() -> PathBuf {
  resolve_working_copy_dir(dir_spec::state_home(), std::env::temp_dir())
}

fn local_working_copy_fallback() -> PathBuf {
  std::env::temp_dir().join("pod").join(WORKING_COPY_SUBDIR)
}

fn resolve_working_copy_dir(state_home: Option<PathBuf>, fallback_root: PathBuf) -> PathBuf {
  state_home
    .unwrap_or(fallback_root)
    .join("pod")
    .join(WORKING_COPY_SUBDIR)
}

fn default_calendar_density() -> CalendarDensity {
  CalendarDensity::default()
}

fn default_calendar_week_start() -> CalendarWeekStart {
  CalendarWeekStart::Sunday
}

fn default_eve_client_id() -> String {
  EVE_CLIENT_ID.to_owned()
}

fn default_mcp_port() -> u16 {
  7373
}

fn default_rail_order() -> Vec<Destination> {
  Destination::REORDERABLE.to_vec()
}

/// Deserializes the cascade mode while healing an unknown value (a renamed or removed mode from an
/// older config) back to the default rather than failing the whole load.
fn deserialize_cascade_mode<'de, D>(deserializer: D) -> Result<CascadeMode, D::Error>
where
  D: serde::Deserializer<'de>,
{
  let raw = String::deserialize(deserializer)?;
  Ok(match raw.as_str() {
    "flyout" => CascadeMode::Flyout,
    "none" => CascadeMode::None,
    "sub_rail" => CascadeMode::SubRail,
    _ => CascadeMode::default(),
  })
}

/// Deserializes the rail order while silently dropping ids that are not a known [`Destination`], so a
/// stale id from an older config (e.g. a removed `market`) can't fail the whole load.
fn deserialize_rail_order<'de, D>(deserializer: D) -> Result<Vec<Destination>, D::Error>
where
  D: serde::Deserializer<'de>,
{
  let ids = Vec::<String>::deserialize(deserializer)?;
  Ok(
    ids
      .into_iter()
      .filter_map(|id| {
        Destination::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(&id)).ok()
      })
      .collect(),
  )
}

fn default_reprocessing_yield() -> f64 {
  0.5
}

fn default_scale_100() -> u8 {
  100
}

fn default_true() -> bool {
  true
}

/// Generates an MCP bearer token of the form `pod_mcp_` followed by 40 lowercase hex characters
/// (20 random bytes). The `pod_mcp_` prefix makes the secret self-identifying in logs and configs.
fn gen_token() -> String {
  let mut bytes = [0u8; 20];
  rand::rng().fill_bytes(&mut bytes);
  let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
  format!("pod_mcp_{hex}")
}

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

fn is_default_language(language: &Language) -> bool {
  *language == Language::default()
}

fn is_default_scale(scale: &u8) -> bool {
  *scale == default_scale_100()
}

fn is_not_high_contrast(high_contrast: &bool) -> bool {
  !*high_contrast
}

pub fn load() -> Result<Settings, Error> {
  load_from(&config_path()?)
}

pub fn reprocessing_yield_or_default() -> f64 {
  load()
    .map(|s| s.reprocessing_yield)
    .unwrap_or_else(|_| default_reprocessing_yield())
}

fn load_from(path: &Path) -> Result<Settings, Error> {
  let mut settings: Settings = Figment::from(Serialized::defaults(Settings::default()))
    .merge(Toml::file(path))
    .extract()
    .map_err(|error| Error::Load(Box::new(error)))?;

  settings.ui.sanitize();
  Ok(settings)
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

/// Merges an archived [`Settings`] onto the local one, restoring the user's portable preferences
/// while preserving everything that identifies this machine.
///
/// Restoring a foreign `config.toml` wholesale would point Pod at the source machine's absolute
/// storage paths and hijack its sync/MCP identity, so this transform splits the fields three ways:
///
/// - Portable (restored from the archive, but only when the archived value differs from its default
///   so an archived default never clobbers a local override): `accessibility`, `features`,
///   `industry`, `ui`, `eve_client_id`.
/// - Machine-local (kept from local): the storage path overrides (`db_dir`, `log_dir`, `cache_dir`),
///   the `network` flag, and `log_level`; plus `machine_id` (taken from the archive only when local
///   has none) and the MCP `token` (taken from the archive only when local is empty). The rest of
///   the MCP config (`enabled`, `perms`, `port`) is machine-local automation state and is kept.
/// - Never serialized: `working_copy_dir` is `#[serde(skip)]`; the result resets it to `None` so the
///   live working-copy protection is never redirected via an import.
///
/// Pure value transform: the caller owns persisting the result via [`save`]/`save_to`.
pub fn merge_for_restore(local: &Settings, archived: &Settings) -> Settings {
  let mut merged = local.clone();

  // Portable preferences: restore from the archive unless the archived value is still its default,
  // which would otherwise wipe out a deliberate local override with an unset import.
  if archived.accessibility != AccessibilityConfig::default() {
    merged.accessibility = archived.accessibility;
  }
  if archived.features != FeatureFlags::default() {
    merged.features = archived.features;
  }
  if archived.industry != IndustryConfig::default() {
    merged.industry = archived.industry;
  }
  if archived.ui != UiConfig::default() {
    merged.ui = archived.ui.clone();
  }
  if archived.eve_client_id != default_eve_client_id() {
    merged.eve_client_id = archived.eve_client_id.clone();
  }

  // Machine-local identity stays as `local` (already cloned), with two recovery cases: a freshly
  // imported install with no machine_id / no MCP token adopts the archived value so the user isn't
  // left without one.
  if merged.storage.machine_id.is_none() {
    merged.storage.machine_id = archived.storage.machine_id.clone();
  }
  if merged.mcp.token.is_empty() {
    merged.mcp.token = archived.mcp.token.clone();
  }

  // Never let an import redirect the live working copy onto a foreign / network path.
  merged.storage.working_copy_dir = None;

  merged
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

  mod cascade_mode {
    use super::*;

    mod label {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_resolves_each_variant_to_its_english_text() {
        assert_eq!(CascadeMode::Flyout.label(), "Flyout");
        assert_eq!(CascadeMode::None.label(), "Off");
        assert_eq!(CascadeMode::SubRail.label(), "Sub-rail");
      }
    }
  }

  mod feature {
    use super::*;

    mod noun {
      use super::*;

      #[test]
      fn it_gives_every_feature_a_nonempty_noun() {
        for feature in Feature::ALL {
          assert!(!feature.noun().is_empty(), "{feature:?} must have a noun");
        }
      }
    }
  }

  mod log_level {
    use super::*;

    mod label {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_resolves_each_variant_to_its_english_text() {
        assert_eq!(LogLevel::Normal.label(), "Normal");
        assert_eq!(LogLevel::Quiet.label(), "Quiet");
        assert_eq!(LogLevel::Verbose.label(), "Verbose");
      }
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

    #[test]
    fn it_defaults_every_sub_feature_to_enabled() {
      let flags = FeatureFlags::default();

      assert_eq!(flags.enabled_sub_features(), SubFeature::ALL.to_vec());
    }

    #[test]
    fn a_group_is_enabled_while_any_child_remains_on() {
      let mut flags = FeatureFlags::default();

      flags.set_sub_enabled(SubFeature::Budget, false);

      assert!(
        flags.is_enabled(Feature::Wallet),
        "other Wallet children keep the group on"
      );

      for sub in Feature::Wallet.sub_features() {
        flags.set_sub_enabled(*sub, false);
      }

      assert!(
        !flags.is_enabled(Feature::Wallet),
        "the group is off only when every child is off"
      );
    }

    #[test]
    fn set_enabled_cascades_to_every_child() {
      let mut flags = FeatureFlags::default();

      flags.set_enabled(Feature::AssetTracking, false);

      assert!(
        Feature::AssetTracking
          .sub_features()
          .iter()
          .all(|&sub| !flags.is_sub_enabled(sub)),
        "a group toggle off clears all of its children"
      );
    }

    #[test]
    fn set_sub_enabled_flips_only_the_named_child() {
      let mut flags = FeatureFlags::default();

      flags.set_sub_enabled(SubFeature::Abyssals, false);

      assert!(!flags.is_sub_enabled(SubFeature::Abyssals));
      assert!(flags.is_sub_enabled(SubFeature::Inventory), "siblings are untouched");
    }

    #[test]
    fn disabling_both_journal_and_transactions_auto_disables_budget() {
      let mut flags = FeatureFlags::default();

      flags.set_sub_enabled(SubFeature::Journal, false);
      assert!(
        flags.is_sub_enabled(SubFeature::Budget),
        "one wallet activity source keeps Budget alive"
      );

      flags.set_sub_enabled(SubFeature::Transactions, false);

      assert!(
        !flags.is_sub_enabled(SubFeature::Budget),
        "Budget has no activity to derive from once both Journal and Transactions are off"
      );
    }

    #[test]
    fn budget_cannot_be_enabled_while_both_sources_are_off() {
      let mut flags = FeatureFlags::default();
      flags.set_sub_enabled(SubFeature::Journal, false);
      flags.set_sub_enabled(SubFeature::Transactions, false);

      flags.set_sub_enabled(SubFeature::Budget, true);

      assert!(
        !flags.is_sub_enabled(SubFeature::Budget),
        "enabling Budget is a no-op while both Journal and Transactions are off"
      );
    }

    #[test]
    fn re_enabling_an_activity_source_lets_budget_be_enabled_again() {
      let mut flags = FeatureFlags::default();
      flags.set_sub_enabled(SubFeature::Journal, false);
      flags.set_sub_enabled(SubFeature::Transactions, false);
      assert!(!flags.is_sub_enabled(SubFeature::Budget));

      flags.set_sub_enabled(SubFeature::Journal, true);
      flags.set_sub_enabled(SubFeature::Budget, true);

      assert!(
        flags.is_sub_enabled(SubFeature::Budget),
        "Budget can be turned back on once an activity source returns"
      );
    }

    #[test]
    fn a_legacy_config_with_budget_on_but_no_sources_loads_with_budget_off() {
      let mut flags = FeatureFlags::default();

      // Mirror a hand-edited nested config that turned both activity sources off but left Budget on.
      flags.set_sub_enabled(SubFeature::Journal, false);
      flags.set_sub_enabled(SubFeature::Transactions, false);
      flags.set_sub_enabled(SubFeature::Budget, true);

      assert!(
        !flags.is_sub_enabled(SubFeature::Budget),
        "the coupling self-heals an inconsistent state"
      );
    }

    #[test]
    fn enabled_sub_features_of_lists_only_the_groups_children() {
      let mut flags = FeatureFlags::default();
      flags.set_sub_enabled(SubFeature::Journal, false);

      let wallet = flags.enabled_sub_features_of(Feature::Wallet);

      assert!(!wallet.contains(&SubFeature::Journal));
      assert!(wallet.contains(&SubFeature::Budget));
      assert!(
        wallet.iter().all(|sub| sub.group() == Feature::Wallet),
        "only Wallet children are returned"
      );
    }
  }

  mod sub_feature {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn every_sub_feature_rolls_up_to_a_group_that_owns_it() {
      for sub in SubFeature::ALL {
        assert!(
          sub.group().sub_features().contains(&sub),
          "{sub:?} must be listed under its group {:?}",
          sub.group()
        );
      }
    }

    #[test]
    fn the_groups_partition_every_sub_feature_exactly_once() {
      let mut seen: HashSet<SubFeature> = HashSet::new();

      for feature in Feature::ALL {
        for &sub in feature.sub_features() {
          assert!(seen.insert(sub), "{sub:?} is owned by more than one group");
        }
      }

      assert_eq!(seen.len(), SubFeature::ALL.len(), "every sub-feature has a group");
    }

    #[test]
    fn sub_feature_keys_are_unique_within_a_group() {
      for feature in Feature::ALL {
        let mut keys: HashSet<&str> = HashSet::new();
        for &sub in feature.sub_features() {
          assert!(
            keys.insert(sub.key()),
            "{:?} has a duplicate child key {}",
            feature,
            sub.key()
          );
        }
      }
    }
  }

  mod config_exists_at {
    use super::*;

    #[test]
    fn it_is_false_when_the_file_is_absent() {
      let dir = tempfile::tempdir().unwrap();

      assert!(!config_exists_at(&dir.path().join("config.toml")));
    }

    #[test]
    fn it_is_false_for_a_directory_at_the_path() {
      let dir = tempfile::tempdir().unwrap();

      assert!(!config_exists_at(dir.path()));
    }

    #[test]
    fn it_is_true_when_the_file_is_present() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "reprocessing_yield = 0.5\n").unwrap();

      assert!(config_exists_at(&path));
    }

    #[test]
    fn it_does_not_create_the_file_as_a_side_effect() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");

      let exists = config_exists_at(&path);

      assert!(!exists);
      assert!(!path.exists(), "the predicate must not materialize the file");
    }
  }

  mod database_exists_at {
    use super::*;

    #[test]
    fn it_is_false_when_neither_database_is_present() {
      let dir = tempfile::tempdir().unwrap();

      assert!(!database_exists_at(
        &dir.path().join("pod.db"),
        &dir.path().join("pod-working.db")
      ));
    }

    #[test]
    fn it_is_true_when_the_canonical_database_is_present() {
      let dir = tempfile::tempdir().unwrap();
      let canonical = dir.path().join("pod.db");
      std::fs::write(&canonical, b"db").unwrap();

      assert!(database_exists_at(&canonical, &dir.path().join("pod-working.db")));
    }

    #[test]
    fn it_is_true_when_only_a_working_copy_is_present() {
      let dir = tempfile::tempdir().unwrap();
      let working_copy = dir.path().join("pod-working.db");
      std::fs::write(&working_copy, b"db").unwrap();

      assert!(database_exists_at(&dir.path().join("pod.db"), &working_copy));
    }

    #[test]
    fn it_does_not_create_the_database_as_a_side_effect() {
      let dir = tempfile::tempdir().unwrap();
      let canonical = dir.path().join("pod.db");
      let working_copy = dir.path().join("pod-working.db");

      let exists = database_exists_at(&canonical, &working_copy);

      assert!(!exists);
      assert!(!canonical.exists(), "the predicate must not materialize the database");
      assert!(
        !working_copy.exists(),
        "the predicate must not materialize the working copy"
      );
    }
  }

  mod database_exists {
    use super::*;

    fn settings_with_storage_root(root: &Path) -> Settings {
      let mut settings = Settings::default();
      settings.storage_mut().set_db_dir(Some(root.to_path_buf()));
      settings.storage_mut().set_working_copy_dir(Some(root.join("working")));
      settings
    }

    #[test]
    fn it_is_false_against_an_empty_storage_root() {
      let dir = tempfile::tempdir().unwrap();
      let settings = settings_with_storage_root(dir.path());

      assert!(!database_exists(&settings));
    }

    #[test]
    fn it_is_true_once_the_resolved_database_is_present() {
      let dir = tempfile::tempdir().unwrap();
      let settings = settings_with_storage_root(dir.path());
      std::fs::write(settings.storage().resolved_database_path(), b"db").unwrap();

      assert!(database_exists(&settings));
    }
  }

  mod is_first_run {
    use super::*;

    #[test]
    fn it_is_true_when_neither_config_nor_database_is_present() {
      assert!(is_first_run(false, false));
    }

    #[test]
    fn it_is_false_when_a_config_is_present() {
      assert!(!is_first_run(true, false));
    }

    #[test]
    fn it_is_false_when_a_database_is_present() {
      assert!(!is_first_run(false, true));
    }

    #[test]
    fn it_is_false_when_both_are_present() {
      assert!(!is_first_run(true, true));
    }
  }

  mod load_from {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_every_feature_flag_to_enabled_when_the_file_is_absent() {
      let dir = tempfile::tempdir().unwrap();

      let settings = load_from(&dir.path().join("config.toml")).unwrap();

      assert_eq!(settings.features().enabled(), Feature::ALL.to_vec());
    }

    #[test]
    fn it_defaults_the_accessibility_table_when_the_file_is_absent() {
      let dir = tempfile::tempdir().unwrap();

      let settings = load_from(&dir.path().join("config.toml")).unwrap();
      let accessibility = settings.accessibility();

      assert_eq!(*accessibility.scale(), 100);
      assert!(!accessibility.high_contrast());
      assert_eq!(accessibility.language(), Language::EnUs);
    }

    #[test]
    fn it_defaults_the_language_when_the_accessibility_table_omits_it() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "[accessibility]\nscale = 125\n").unwrap();

      let accessibility = load_from(&path).unwrap().accessibility().to_owned();

      assert_eq!(accessibility.language(), Language::EnUs);
    }

    #[test]
    fn it_reads_a_language_override() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "[accessibility]\nlanguage = \"de\"\n").unwrap();

      let accessibility = load_from(&path).unwrap().accessibility().to_owned();

      assert_eq!(accessibility.language(), Language::De);
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
    fn it_defaults_the_reprocessing_yield_when_the_file_is_absent() {
      let dir = tempfile::tempdir().unwrap();

      let settings = load_from(&dir.path().join("config.toml")).unwrap();

      assert_eq!(*settings.reprocessing_yield(), 0.5);
    }

    #[test]
    fn it_reads_a_reprocessing_yield_override() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "reprocessing_yield = 0.78\n").unwrap();

      let settings = load_from(&path).unwrap();

      assert_eq!(*settings.reprocessing_yield(), 0.78);
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
    fn it_reads_a_partial_accessibility_table_and_defaults_the_rest() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "[accessibility]\nscale = 125\n").unwrap();

      let accessibility = load_from(&path).unwrap().accessibility().to_owned();

      assert_eq!(*accessibility.scale(), 125);
      assert!(!accessibility.high_contrast());
    }

    #[test]
    fn it_reads_feature_overrides_and_keeps_unlisted_flags_enabled() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "[features]\nwallet = false\nmail = false\n").unwrap();

      let features = load_from(&path).unwrap().features().to_owned();

      assert!(!features.is_enabled(Feature::Wallet));
      assert!(!features.is_enabled(Feature::Mail));
      assert!(features.is_enabled(Feature::CloneMonitoring));
      assert!(features.is_enabled(Feature::Contacts));
      assert!(!features.enabled().contains(&Feature::Wallet));
    }

    #[test]
    fn it_migrates_a_legacy_all_true_flat_config_to_every_sub_feature_on() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(
        &path,
        "[features]\nwallet = true\nindustry = true\nasset_tracking = true\n",
      )
      .unwrap();

      let features = load_from(&path).unwrap().features().to_owned();

      assert_eq!(
        features,
        FeatureFlags::default(),
        "an all-true legacy file is the all-on default"
      );
      assert_eq!(features.enabled_sub_features(), SubFeature::ALL.to_vec());
    }

    #[test]
    fn it_cascades_a_legacy_group_false_onto_every_child() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "[features]\nwallet = false\n").unwrap();

      let features = load_from(&path).unwrap().features().to_owned();

      for sub in Feature::Wallet.sub_features() {
        assert!(
          !features.is_sub_enabled(*sub),
          "{sub:?} must be off under a legacy `wallet = false`"
        );
      }
      assert!(
        features.is_enabled(Feature::AssetTracking),
        "untouched groups stay fully on"
      );
    }

    #[test]
    fn it_reads_a_new_nested_partial_config() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "[features.wallet]\nbudget = false\n").unwrap();

      let features = load_from(&path).unwrap().features().to_owned();

      assert!(!features.is_sub_enabled(SubFeature::Budget));
      assert!(
        features.is_sub_enabled(SubFeature::Journal),
        "unlisted Wallet children stay on"
      );
      assert!(
        features.is_enabled(Feature::Wallet),
        "the group is still on via its other children"
      );
    }

    #[test]
    fn it_loads_a_mixed_flat_and_nested_config() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(
        &path,
        "[features]\nmail = false\n\n[features.industry]\nplanner = false\n",
      )
      .unwrap();

      let features = load_from(&path).unwrap().features().to_owned();

      assert!(
        !features.is_enabled(Feature::Mail),
        "the flat key cascades the group off"
      );
      assert!(
        !features.is_sub_enabled(SubFeature::Planner),
        "the nested key flips one child"
      );
      assert!(
        features.is_sub_enabled(SubFeature::Blueprints),
        "unlisted Industry children stay on"
      );
    }

    #[test]
    fn it_defaults_a_brand_new_sub_feature_on_for_a_legacy_config_that_never_mentioned_it() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "[features]\nwallet = true\n").unwrap();

      let features = load_from(&path).unwrap().features().to_owned();

      assert!(
        features.is_sub_enabled(SubFeature::Budget),
        "a sub-feature absent from the legacy file defaults to enabled"
      );
    }

    #[test]
    fn a_new_nested_config_round_trips_through_save_and_load() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      let mut settings = Settings::default();
      settings.features_mut().set_sub_enabled(SubFeature::Budget, false);
      settings.features_mut().set_sub_enabled(SubFeature::Abyssals, false);

      save_to(&path, &settings).unwrap();
      let loaded = load_from(&path).unwrap();

      assert_eq!(loaded.features(), settings.features());
      let serialized = std::fs::read_to_string(&path).unwrap();
      assert!(
        serialized.contains("[features.wallet]"),
        "the next save re-serializes in the nested form: {serialized}"
      );
      assert!(serialized.contains("budget = false"), "{serialized}");
    }

    #[test]
    fn it_forces_budget_off_when_a_nested_config_disables_both_activity_sources() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(
        &path,
        "[features.wallet]\njournal = false\ntransactions = false\nbudget = true\n",
      )
      .unwrap();

      let features = load_from(&path).unwrap().features().to_owned();

      assert!(
        !features.is_sub_enabled(SubFeature::Budget),
        "a config that leaves Budget on with both sources off loads with Budget coupled off"
      );
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

    #[test]
    fn it_returns_defaults_when_the_file_is_absent() {
      let dir = tempfile::tempdir().unwrap();

      let settings = load_from(&dir.path().join("config.toml")).unwrap();

      assert_eq!(settings.eve_client_id(), EVE_CLIENT_ID);
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

  mod mcp_config {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_off_with_the_canonical_port_and_perms() {
      let config = McpConfig::default();

      assert!(!config.enabled());
      assert_eq!(*config.port(), 7373);
      assert!(config.token().is_empty());
      assert!(config.perms().read());
      assert!(config.perms().local_write());
      assert!(!config.perms().send_mail());
      assert!(!config.perms().delete_mail());
      assert!(!config.perms().manage_labels());
    }

    #[test]
    fn token_or_generate_mints_a_prefixed_token_once() {
      let mut config = McpConfig::default();

      let first = config.token_or_generate();
      let second = config.token_or_generate();

      assert!(first.starts_with("pod_mcp_"), "{first}");
      assert_eq!(first.len(), "pod_mcp_".len() + 40);
      assert_eq!(first, second, "a present token is not regenerated");
    }

    #[test]
    fn the_table_is_omitted_from_the_file_while_default() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");

      save_to(&path, &Settings::default()).unwrap();
      let serialized = std::fs::read_to_string(&path).unwrap();

      assert!(
        !serialized.contains("[mcp]"),
        "a default mcp config is skipped: {serialized}"
      );
    }

    #[test]
    fn it_round_trips_a_non_default_config_through_the_file() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      let mut settings = Settings::default();
      settings.mcp_mut().set_enabled(true);
      settings.mcp_mut().set_port(9999);
      let token = settings.mcp_mut().token_or_generate();
      let mut perms = *settings.mcp().perms();
      perms.set_send_mail(true);
      settings.mcp_mut().set_perms(perms);

      save_to(&path, &settings).unwrap();
      let loaded = load_from(&path).unwrap();

      assert!(loaded.mcp().enabled());
      assert_eq!(*loaded.mcp().port(), 9999);
      assert_eq!(loaded.mcp().token(), &token);
      assert!(loaded.mcp().perms().send_mail());
    }

    #[test]
    fn it_auto_generates_a_token_on_load_when_one_is_enabled_without_a_token() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "[mcp]\nenabled = true\n").unwrap();

      let mut loaded = load_from(&path).unwrap();
      let token = loaded.mcp_mut().token_or_generate();

      assert!(token.starts_with("pod_mcp_"), "{token}");
      assert!(loaded.mcp().enabled());
    }
  }

  mod telemetry_config {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_defaults_every_stream_on() {
      let config = TelemetryConfig::default();

      assert!(config.enabled());
      assert!(config.usage());
      assert!(config.performance());
      assert!(config.crashes());
      assert!(config.environment());
    }

    #[test]
    fn the_table_is_omitted_from_the_file_while_default() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");

      save_to(&path, &Settings::default()).unwrap();
      let serialized = std::fs::read_to_string(&path).unwrap();

      assert!(
        !serialized.contains("[telemetry]"),
        "a default telemetry config is skipped: {serialized}"
      );
    }

    #[test]
    fn it_round_trips_a_non_default_config_through_the_file() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      let mut settings = Settings::default();
      settings.telemetry_mut().set_enabled(false);
      settings.telemetry_mut().set_usage(false);
      settings.telemetry_mut().set_crashes(false);

      save_to(&path, &settings).unwrap();
      let serialized = std::fs::read_to_string(&path).unwrap();
      let loaded = load_from(&path).unwrap();

      assert!(
        serialized.contains("enabled = false"),
        "opting out must persist the explicit false: {serialized}"
      );
      assert!(!loaded.telemetry().enabled());
      assert!(!loaded.telemetry().usage());
      assert!(!loaded.telemetry().crashes());
      assert!(loaded.telemetry().performance());
      assert!(loaded.telemetry().environment());
    }

    #[test]
    fn a_legacy_config_without_the_table_loads_opted_in() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "eve_client_id = \"abc\"\n").unwrap();

      let loaded = load_from(&path).unwrap();

      assert!(loaded.telemetry().enabled());
      assert!(loaded.telemetry().usage());
      assert!(loaded.telemetry().performance());
      assert!(loaded.telemetry().crashes());
      assert!(loaded.telemetry().environment());
    }

    #[test]
    fn a_partial_table_fills_missing_streams_as_on() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      std::fs::write(&path, "[telemetry]\nusage = false\n").unwrap();

      let loaded = load_from(&path).unwrap();

      assert!(!loaded.telemetry().usage());
      assert!(loaded.telemetry().enabled());
      assert!(loaded.telemetry().performance());
      assert!(loaded.telemetry().crashes());
      assert!(loaded.telemetry().environment());
    }

    #[test]
    fn the_table_uses_a_flat_shape_with_no_nested_streams() {
      let mut config = TelemetryConfig::default();
      config.set_usage(false);

      let toml = toml::to_string_pretty(&config).unwrap();

      assert!(
        !toml.contains("[streams]"),
        "the streams must be flat fields, not a nested table: {toml}"
      );
      assert!(
        toml.contains("usage = false"),
        "a flat usage field must persist: {toml}"
      );
      let restored: TelemetryConfig = toml::from_str(&toml).unwrap();
      assert_eq!(restored, config);
    }
  }

  mod resolve_data_dir {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_the_given_root_when_data_home_is_missing() {
      let resolved = resolve_data_dir(None, PathBuf::from("/var/tmp"));

      assert_eq!(resolved, PathBuf::from("/var/tmp/pod"));
    }

    #[test]
    fn it_uses_the_data_home_when_present() {
      let resolved = resolve_data_dir(Some(PathBuf::from("/home/me/.local/share")), PathBuf::from("/tmp"));

      assert_eq!(resolved, PathBuf::from("/home/me/.local/share/pod"));
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
    fn it_falls_back_to_the_given_root_when_state_home_is_missing() {
      let resolved = resolve_log_dir(None, PathBuf::from("/var/tmp"));

      assert_eq!(resolved, PathBuf::from("/var/tmp/pod/logs"));
    }

    #[test]
    fn it_uses_the_state_home_when_present() {
      let resolved = resolve_log_dir(Some(PathBuf::from("/home/me/.local/state")), PathBuf::from("/tmp"));

      assert_eq!(resolved, PathBuf::from("/home/me/.local/state/pod/logs"));
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

  mod resolved_paths {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_the_cache_override() {
      let mut storage = StorageConfig::default();
      storage.set_cache_dir(Some(PathBuf::from("/var/pod/cache")));

      assert_eq!(storage.resolved_cache_dir(), PathBuf::from("/var/pod/cache"));
    }

    #[test]
    fn it_resolves_the_log_override() {
      let mut storage = StorageConfig::default();
      storage.set_log_dir(Some(PathBuf::from("/var/pod/log")));

      assert_eq!(storage.resolved_log_dir(), PathBuf::from("/var/pod/log"));
    }

    #[test]
    fn it_uses_the_db_dir_override_for_the_database_path() {
      let mut storage = StorageConfig::default();
      storage.set_db_dir(Some(PathBuf::from("/var/pod/db")));

      assert_eq!(storage.resolved_db_dir(), PathBuf::from("/var/pod/db"));
      assert_eq!(storage.resolved_database_path(), PathBuf::from("/var/pod/db/pod.db"));
    }

    #[test]
    fn it_uses_the_platform_default_cache_dir_with_no_override() {
      let storage = StorageConfig::default();

      assert_eq!(storage.resolved_cache_dir(), cache_dir());
    }

    #[test]
    fn it_uses_the_platform_default_database_path_with_no_override() {
      let storage = StorageConfig::default();

      assert_eq!(storage.resolved_database_path(), data_dir().join("pod.db"));
    }

    #[test]
    fn it_uses_the_state_home_default_log_dir_with_no_override() {
      let storage = StorageConfig::default();

      assert_eq!(storage.resolved_log_dir(), log_dir());
    }
  }

  mod resolved_working_copy_path {
    use pretty_assertions::assert_eq;

    use super::*;

    fn always(kind: FsKind) -> impl Fn(&Path) -> FsKind {
      move |_| kind
    }

    #[test]
    fn it_is_distinct_from_the_shared_db_path() {
      let mut storage = StorageConfig::default();
      storage.set_db_dir(Some(PathBuf::from("/mnt/nas/pod")));

      assert_ne!(storage.resolved_working_copy_path(), storage.resolved_database_path());
    }

    #[test]
    fn it_keeps_a_local_base_in_place() {
      let mut storage = StorageConfig::default();
      storage.set_working_copy_dir(Some(PathBuf::from("/var/local/wc")));

      let dir = storage.resolved_working_copy_dir(always(FsKind::Local));

      assert_eq!(dir, PathBuf::from("/var/local/wc"));
    }

    #[test]
    fn it_redirects_to_a_local_fallback_when_the_base_is_on_a_network_fs() {
      let mut storage = StorageConfig::default();
      storage.set_working_copy_dir(Some(PathBuf::from("/mnt/nas/wc")));

      let dir = storage.resolved_working_copy_dir(always(FsKind::Network));

      assert_eq!(
        dir,
        local_working_copy_fallback(),
        "a network working-copy base is rejected for the local fallback"
      );
    }

    #[test]
    fn it_stays_off_the_cache_dir_even_when_cache_points_at_a_network_path() {
      let mut storage = StorageConfig::default();
      storage.set_cache_dir(Some(PathBuf::from("/mnt/nas/cache")));

      let path = storage.resolved_working_copy_path();

      assert!(
        !path.starts_with("/mnt/nas/cache"),
        "the live working copy is never placed under the configurable (evictable, network-capable) cache_dir"
      );
      assert_eq!(path.file_name().unwrap(), "pod.db");
    }
  }

  mod save_to {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_roundtrips_a_non_default_accessibility_table() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      let mut settings = Settings::default();
      settings.accessibility_mut().set_scale(125);
      settings.accessibility_mut().set_high_contrast(true);

      save_to(&path, &settings).unwrap();
      let loaded = load_from(&path).unwrap();

      assert_eq!(loaded.accessibility(), settings.accessibility());
      assert_eq!(*loaded.accessibility().scale(), 125);
      assert!(loaded.accessibility().high_contrast());
    }

    #[test]
    fn it_roundtrips_the_feature_and_storage_tables() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("config.toml");
      let mut settings = Settings::default();
      settings.features_mut().set_enabled(Feature::Wallet, false);
      settings.features_mut().set_enabled(Feature::CombatLog, false);
      settings.storage.network = true;
      settings.storage.log_dir = Some(PathBuf::from("/tmp/pod-logs"));

      save_to(&path, &settings).unwrap();
      let loaded = load_from(&path).unwrap();

      assert_eq!(loaded.features(), settings.features());
      assert_eq!(loaded.storage(), settings.storage());
      assert!(!loaded.features().is_enabled(Feature::Wallet));
      assert!(loaded.storage().network());
      assert_eq!(*loaded.storage().log_dir(), Some(PathBuf::from("/tmp/pod-logs")));
    }

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
  }

  mod select_resource_dir {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_prefers_the_resources_bundle_over_the_exe_dir() {
      let exe_dir = PathBuf::from("/opt/pod");
      let resources = exe_dir.join("../Resources");

      let resolved = select_resource_dir(&exe_dir, Some("pod"), |path| *path == resources || *path == exe_dir);

      assert_eq!(resolved, Some(resources));
    }

    #[test]
    fn it_returns_none_when_no_candidate_holds_the_assets() {
      let exe_dir = PathBuf::from("/usr/bin");

      let resolved = select_resource_dir(&exe_dir, Some("pod"), |_| false);

      assert_eq!(resolved, None);
    }

    #[test]
    fn it_selects_the_exe_dir_for_the_windows_layout() {
      let exe_dir = PathBuf::from("C:/Program Files/pod");

      let resolved = select_resource_dir(&exe_dir, Some("pod"), |path| *path == exe_dir);

      assert_eq!(resolved, Some(exe_dir));
    }

    #[test]
    fn it_selects_the_linux_lib_dir_for_the_fhs_layout() {
      let exe_dir = PathBuf::from("/usr/bin");
      let lib_dir = exe_dir.join("../lib").join("pod");

      let resolved = select_resource_dir(&exe_dir, Some("pod"), |path| *path == lib_dir);

      assert_eq!(resolved, Some(PathBuf::from("/usr/bin/../lib/pod")));
    }

    #[test]
    fn it_selects_the_macos_resources_bundle() {
      let exe_dir = PathBuf::from("/Applications/pod.app/Contents/MacOS");
      let resources = exe_dir.join("../Resources");

      let resolved = select_resource_dir(&exe_dir, Some("pod"), |path| *path == resources);

      assert_eq!(resolved, Some(resources));
    }

    #[test]
    fn it_skips_the_linux_candidate_when_the_binary_name_is_unknown() {
      let exe_dir = PathBuf::from("/usr/bin");
      let lib_dir = exe_dir.join("../lib").join("pod");

      let resolved = select_resource_dir(&exe_dir, None, |path| *path == lib_dir);

      assert_eq!(resolved, None);
    }
  }

  mod serialization {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn a_default_accessibility_config_serializes_without_any_keys() {
      let toml = toml::to_string_pretty(&AccessibilityConfig::default()).unwrap();

      assert!(!toml.contains("scale"), "a default scale must not leak to disk: {toml}");
      assert!(
        !toml.contains("high_contrast"),
        "a default high_contrast must not leak to disk: {toml}"
      );
      assert!(
        !toml.contains("language"),
        "a default (en-us) language must not leak to disk: {toml}"
      );
    }

    #[test]
    fn a_non_default_language_round_trips_through_toml() {
      let mut accessibility = AccessibilityConfig::default();
      accessibility.set_language(Language::De);

      let toml = toml::to_string_pretty(&accessibility).unwrap();
      let restored: AccessibilityConfig = toml::from_str(&toml).unwrap();

      assert!(
        toml.contains("language = \"de\""),
        "a non-default language must persist as its ESI code: {toml}"
      );
      assert_eq!(restored.language(), Language::De);
    }

    #[test]
    fn a_default_industry_config_serializes_without_any_keys() {
      let toml = toml::to_string_pretty(&IndustryConfig::default()).unwrap();

      assert!(
        !toml.contains("manufacturing"),
        "an unset manufacturing facility must not leak to disk: {toml}"
      );
      assert!(
        !toml.contains("reactions"),
        "an unset reactions facility must not leak to disk: {toml}"
      );
    }

    #[test]
    fn a_default_settings_serializes_without_an_accessibility_table() {
      let toml = toml::to_string_pretty(&Settings::default()).unwrap();

      assert!(
        !toml.contains("[accessibility]"),
        "a default accessibility table must not leak to disk: {toml}"
      );
      assert!(!toml.contains("scale"), "a default scale must not leak to disk: {toml}");
      assert!(
        !toml.contains("high_contrast"),
        "a default high_contrast must not leak to disk: {toml}"
      );
    }

    #[test]
    fn a_default_settings_serializes_without_an_industry_table() {
      let toml = toml::to_string_pretty(&Settings::default()).unwrap();

      assert!(
        !toml.contains("[industry]"),
        "a default industry table must not leak to disk: {toml}"
      );
    }

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
      assert!(
        !toml.contains("log_level"),
        "a default (Quiet) log_level must not leak to disk: {toml}"
      );
    }

    #[test]
    fn a_non_default_log_level_round_trips_through_toml() {
      let mut storage = StorageConfig::default();
      storage.set_log_level(LogLevel::Verbose);

      let toml = toml::to_string_pretty(&storage).unwrap();
      let restored: StorageConfig = toml::from_str(&toml).unwrap();

      assert!(
        toml.contains("log_level = \"verbose\""),
        "a non-default log_level must persist in snake_case: {toml}"
      );
      assert_eq!(restored.log_level(), &LogLevel::Verbose);
    }

    #[test]
    fn a_partially_customized_accessibility_config_only_writes_the_changed_keys() {
      let mut accessibility = AccessibilityConfig::default();
      accessibility.set_scale(125);

      let toml = toml::to_string_pretty(&accessibility).unwrap();

      assert!(toml.contains("scale = 125"), "scale override must persist: {toml}");
      assert!(
        !toml.contains("high_contrast"),
        "a default high_contrast must not leak to disk: {toml}"
      );
    }

    #[test]
    fn an_industry_config_with_one_activity_set_round_trips_through_toml() {
      let mut industry = IndustryConfig::default();
      industry.set_manufacturing(Some(60003760));

      let toml = toml::to_string_pretty(&industry).unwrap();
      let restored: IndustryConfig = toml::from_str(&toml).unwrap();

      assert!(
        toml.contains("manufacturing = 60003760"),
        "a set manufacturing facility must persist: {toml}"
      );
      assert!(
        !toml.contains("reactions"),
        "an unset reactions facility must not leak to disk: {toml}"
      );
      assert_eq!(restored, industry);
    }
  }

  mod storage_mode {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_direct_with_the_opt_in_flag_off() {
      let storage = StorageConfig::default();

      assert_eq!(storage.storage_mode(), StorageMode::Direct);
    }

    #[test]
    fn it_is_sync_only_when_the_opt_in_flag_is_set() {
      let mut storage = StorageConfig::default();
      storage.set_network(true);

      assert_eq!(storage.storage_mode(), StorageMode::Sync);
    }

    #[test]
    fn it_stays_direct_for_a_network_db_dir_when_the_opt_in_flag_is_off() {
      let mut storage = StorageConfig::default();
      storage.set_db_dir(Some(PathBuf::from("/mnt/nas/pod")));

      assert_eq!(
        storage.storage_mode(),
        StorageMode::Direct,
        "a network FS no longer auto-flips Sync; entering Sync is opt-in only"
      );
    }
  }

  mod suggests_network_sync {
    use pretty_assertions::assert_eq;

    use super::*;

    fn always(kind: FsKind) -> impl Fn(&Path) -> FsKind {
      move |_| kind
    }

    #[test]
    fn it_does_not_suggest_for_a_local_db_dir() {
      let storage = StorageConfig::default();

      assert!(!storage.suggests_network_sync_with(always(FsKind::Local)));
    }

    #[test]
    fn it_does_not_suggest_when_sync_is_already_on() {
      let mut storage = StorageConfig::default();
      storage.set_network(true);

      assert!(!storage.suggests_network_sync_with(always(FsKind::Network)));
    }

    #[test]
    fn it_suggests_sync_when_the_db_dir_is_on_a_network_fs_and_sync_is_off() {
      let mut storage = StorageConfig::default();
      storage.set_db_dir(Some(PathBuf::from("/mnt/nas/pod")));
      let seen = std::cell::RefCell::new(None);

      let suggests = storage.suggests_network_sync_with(|path| {
        *seen.borrow_mut() = Some(path.to_path_buf());
        FsKind::Network
      });

      assert!(suggests, "the advisory fires for a network db_dir while in Direct mode");
      assert_eq!(seen.into_inner(), Some(PathBuf::from("/mnt/nas/pod")));
    }
  }

  mod ui_config {
    use super::*;

    mod is_default {
      use super::*;

      #[test]
      fn it_is_true_for_an_untouched_config() {
        assert!(UiConfig::default().is_default());
      }

      #[test]
      fn it_is_false_once_the_nav_location_moves() {
        let mut ui = UiConfig::default();
        ui.set_nav_location(NavLocation::Right);

        assert!(!ui.is_default());
      }

      #[test]
      fn it_is_false_once_the_cascade_mode_moves() {
        let mut ui = UiConfig::default();
        ui.set_cascade_mode(CascadeMode::None);

        assert!(!ui.is_default());
      }
    }

    mod cascade_mode {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_defaults_to_flyout() {
        assert_eq!(*UiConfig::default().cascade_mode(), CascadeMode::Flyout);
      }

      #[test]
      fn it_deserializes_each_known_mode() {
        for (raw, expected) in [
          ("flyout", CascadeMode::Flyout),
          ("sub_rail", CascadeMode::SubRail),
          ("none", CascadeMode::None),
        ] {
          let toml = format!("cascade_mode = \"{raw}\"\nnav_location = \"left\"\nrail_order = []\n");
          let ui: UiConfig = toml::from_str(&toml).unwrap();

          assert_eq!(*ui.cascade_mode(), expected, "mode `{raw}` must deserialize");
        }
      }

      #[test]
      fn it_heals_an_unknown_mode_back_to_the_default() {
        let toml = "cascade_mode = \"carousel\"\nnav_location = \"left\"\nrail_order = []\n";

        let ui: UiConfig = toml::from_str(toml).unwrap();

        assert_eq!(*ui.cascade_mode(), CascadeMode::Flyout);
      }

      #[test]
      fn it_round_trips_each_mode_through_toml() {
        for mode in CascadeMode::ALL {
          let mut ui = UiConfig::default();
          ui.set_cascade_mode(mode);

          let toml = toml::to_string_pretty(&ui).unwrap();
          let restored: UiConfig = toml::from_str(&toml).unwrap();

          assert_eq!(*restored.cascade_mode(), mode);
        }
      }
    }

    mod sanitize {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_appends_missing_known_destinations_in_default_order() {
        let mut ui = UiConfig::default();
        ui.set_rail_order(vec![Destination::Wallet, Destination::Mail]);

        ui.sanitize();

        assert_eq!(
          *ui.rail_order(),
          vec![
            Destination::Wallet,
            Destination::Mail,
            Destination::Characters,
            Destination::Skills,
            Destination::Industry,
            Destination::Calendar,
            Destination::Assets,
          ]
        );
      }

      #[test]
      fn it_drops_duplicates_keeping_the_first_occurrence() {
        let mut ui = UiConfig::default();
        ui.set_rail_order(vec![Destination::Assets, Destination::Assets]);

        ui.sanitize();

        assert_eq!(ui.rail_order().iter().filter(|&&d| d == Destination::Assets).count(), 1);
        assert_eq!(ui.rail_order()[0], Destination::Assets);
      }

      #[test]
      fn it_drops_the_pinned_settings_destination() {
        let mut ui = UiConfig::default();
        ui.set_rail_order(vec![Destination::Settings, Destination::Characters]);

        ui.sanitize();

        assert!(!ui.rail_order().contains(&Destination::Settings));
      }

      #[test]
      fn it_heals_an_empty_order_to_the_full_default() {
        let mut ui = UiConfig::default();
        ui.set_rail_order(Vec::new());

        ui.sanitize();

        assert_eq!(*ui.rail_order(), Destination::REORDERABLE.to_vec());
      }

      #[test]
      fn it_preserves_the_relative_order_of_known_items() {
        let mut ui = UiConfig::default();
        ui.set_rail_order(vec![
          Destination::Calendar,
          Destination::Characters,
          Destination::Wallet,
        ]);

        ui.sanitize();

        assert_eq!(ui.rail_order()[0], Destination::Calendar);
        assert_eq!(ui.rail_order()[1], Destination::Characters);
        assert_eq!(ui.rail_order()[2], Destination::Wallet);
      }
    }

    mod serialization {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn a_customized_config_round_trips_through_toml() {
        let mut ui = UiConfig::default();
        ui.set_nav_location(NavLocation::Right);
        ui.set_rail_order(vec![Destination::Assets, Destination::Mail]);

        let toml = toml::to_string_pretty(&ui).unwrap();
        let restored: UiConfig = toml::from_str(&toml).unwrap();

        assert_eq!(restored, ui);
        assert!(
          toml.contains("nav_location = \"right\""),
          "nav_location must persist in snake_case: {toml}"
        );
      }

      #[test]
      fn a_default_settings_serializes_without_a_ui_table() {
        let toml = toml::to_string_pretty(&Settings::default()).unwrap();

        assert!(
          !toml.contains("[ui]"),
          "a default ui table must not leak to disk: {toml}"
        );
      }

      #[test]
      fn it_drops_an_unknown_destination_on_load_via_serialized_order() {
        let toml = "[ui]\nrail_order = [\"market\", \"characters\"]\n";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, toml).unwrap();

        let ui = load_from(&path).unwrap().ui().to_owned();

        assert!(!ui.rail_order().is_empty());
        assert_eq!(*ui.rail_order(), Destination::REORDERABLE.to_vec());
      }

      #[test]
      fn it_round_trips_through_the_settings_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut settings = Settings::default();
        settings.ui_mut().set_nav_location(NavLocation::Right);
        settings
          .ui_mut()
          .set_rail_order(vec![Destination::Assets, Destination::Wallet]);

        save_to(&path, &settings).unwrap();
        let loaded = load_from(&path).unwrap();

        assert_eq!(loaded.ui().nav_location(), &NavLocation::Right);
        assert_eq!(loaded.ui().rail_order()[0], Destination::Assets);
        assert_eq!(loaded.ui().rail_order()[1], Destination::Wallet);
      }
    }
  }

  mod merge_for_restore {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_restores_a_portable_field_from_the_archive() {
      let local = Settings::default();
      let mut archived = Settings::default();
      archived.accessibility.set_scale(125);
      archived.accessibility.set_high_contrast(true);

      let merged = merge_for_restore(&local, &archived);

      assert_eq!(*merged.accessibility().scale(), 125);
      assert!(*merged.accessibility().high_contrast());
    }

    #[test]
    fn it_restores_every_portable_field_group() {
      let local = Settings::default();
      let mut archived = Settings::default();
      archived.features.set_enabled(Feature::Wallet, false);
      archived.industry.set_manufacturing(Some(60003760));
      archived.ui.set_nav_location(NavLocation::Right);
      archived.eve_client_id = "imported-client-id".to_string();

      let merged = merge_for_restore(&local, &archived);

      assert!(!merged.features().is_enabled(Feature::Wallet));
      assert_eq!(*merged.industry().manufacturing(), Some(60003760));
      assert_eq!(merged.ui().nav_location(), &NavLocation::Right);
      assert_eq!(merged.eve_client_id(), "imported-client-id");
    }

    #[test]
    fn it_does_not_clobber_a_local_override_with_an_archived_default() {
      let mut local = Settings::default();
      local.accessibility.set_scale(150);
      let archived = Settings::default(); // archived accessibility is still the default

      let merged = merge_for_restore(&local, &archived);

      assert_eq!(
        *merged.accessibility().scale(),
        150,
        "local override survives an archived default"
      );
    }

    #[test]
    fn it_restores_a_non_default_archived_language() {
      let local = Settings::default();
      let mut archived = Settings::default();
      archived.accessibility.set_language(Language::De);

      let merged = merge_for_restore(&local, &archived);

      assert_eq!(merged.accessibility().language(), Language::De);
    }

    #[test]
    fn it_does_not_clobber_a_local_language_override_with_an_archived_en_us_default() {
      let mut local = Settings::default();
      local.accessibility.set_language(Language::Fr);
      let archived = Settings::default(); // archived language is still the en-us default

      let merged = merge_for_restore(&local, &archived);

      assert_eq!(
        merged.accessibility().language(),
        Language::Fr,
        "local language override survives an archived en-us default"
      );
    }

    #[test]
    fn it_preserves_local_machine_identity_and_paths() {
      let mut local = Settings::default();
      local.storage.set_machine_id(Some("local-machine".to_string()));
      local.storage.set_db_dir(Some(PathBuf::from("/var/pod/db")));
      local.storage.set_log_dir(Some(PathBuf::from("/var/pod/log")));
      local.storage.set_cache_dir(Some(PathBuf::from("/var/pod/cache")));
      local.storage.set_network(true);
      local.storage.set_log_level(LogLevel::Verbose);
      local.mcp.set_token("local-token".to_string());

      let mut archived = Settings::default();
      archived.storage.set_machine_id(Some("foreign-machine".to_string()));
      archived.storage.set_db_dir(Some(PathBuf::from("/mnt/nas/db")));
      archived.storage.set_log_dir(Some(PathBuf::from("/mnt/nas/log")));
      archived.storage.set_cache_dir(Some(PathBuf::from("/mnt/nas/cache")));
      archived.storage.set_network(false);
      archived.storage.set_log_level(LogLevel::Normal);
      archived.mcp.set_token("foreign-token".to_string());

      let merged = merge_for_restore(&local, &archived);

      assert_eq!(merged.storage().machine_id().as_deref(), Some("local-machine"));
      assert_eq!(merged.storage().db_dir().as_deref(), Some(Path::new("/var/pod/db")));
      assert_eq!(merged.storage().log_dir().as_deref(), Some(Path::new("/var/pod/log")));
      assert_eq!(
        merged.storage().cache_dir().as_deref(),
        Some(Path::new("/var/pod/cache"))
      );
      assert!(*merged.storage().network());
      assert_eq!(*merged.storage().log_level(), LogLevel::Verbose);
      assert_eq!(merged.mcp().token(), "local-token");
    }

    #[test]
    fn it_adopts_the_archived_machine_id_when_local_has_none() {
      let local = Settings::default(); // machine_id is None
      let mut archived = Settings::default();
      archived.storage.set_machine_id(Some("foreign-machine".to_string()));

      let merged = merge_for_restore(&local, &archived);

      assert_eq!(merged.storage().machine_id().as_deref(), Some("foreign-machine"));
    }

    #[test]
    fn it_adopts_the_archived_mcp_token_when_local_is_empty() {
      let local = Settings::default(); // token is empty
      let mut archived = Settings::default();
      archived.mcp.set_token("foreign-token".to_string());

      let merged = merge_for_restore(&local, &archived);

      assert_eq!(merged.mcp().token(), "foreign-token");
    }

    #[test]
    fn it_resets_the_working_copy_dir() {
      let mut local = Settings::default();
      local.storage.working_copy_dir = Some(PathBuf::from("/var/pod/db/working"));
      let mut archived = Settings::default();
      archived.storage.working_copy_dir = Some(PathBuf::from("/mnt/nas/working"));

      let merged = merge_for_restore(&local, &archived);

      assert_eq!(
        merged.storage.working_copy_dir, None,
        "working_copy_dir is never carried across a merge"
      );
    }
  }
}
