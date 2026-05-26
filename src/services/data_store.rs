//! Disk-backed in-memory data store for shared application state.
//!
//! `DataStore` holds all synced ESI data across all authenticated characters
//! and survives app restarts. Data is stored at
//! `{state_home}/pod/<data_type>/<character_id>.bin` using JSON encoding.
//!
//! On startup `DataStore::load()` reads every persisted file; missing files
//! yield empty collections and are never treated as errors. Each `set_*`
//! write method updates the in-memory cache and immediately flushes to disk.

use std::{collections::HashMap, path::PathBuf};

use pod_model::AttrKey;
use pod_ui::views::{
  assets::AssetRecord,
  mail::MailMessage,
  wallet::{ContractEntry, JournalEntry, MarketEntry},
};
use serde::{Deserialize, Serialize};

/// Shared in-memory store populated by `SyncService`.
pub struct DataStore {
  assets: HashMap<i64, Vec<AssetRecord>>,
  contracts: HashMap<i64, Vec<ContractEntry>>,
  mail: HashMap<i64, Vec<MailMessage>>,
  skills: HashMap<i64, Vec<pod_model::SkillGroupDef>>,
  wallet_journal: HashMap<i64, Vec<JournalEntry>>,
  wallet_transactions: HashMap<i64, Vec<MarketEntry>>,
}

impl DataStore {
  /// Loads (or initialises) the data store from persistent storage.
  ///
  /// Missing files produce empty collections; no error is ever returned.
  pub fn load() -> Self {
    let assets = load_map("assets", |s: StoredAssetRecord| AssetRecord::from(s));
    let contracts = load_map("contracts", |s: StoredContractEntry| ContractEntry::from(s));
    let mail = load_map("mail", |s: StoredMailMessage| MailMessage::from(s));
    let skills = load_map("skills", |s: StoredSkillGroupDef| s.into_model());
    let wallet_journal = load_map("wallet_journal", |s: StoredJournalEntry| JournalEntry::from(s));
    let wallet_transactions = load_map("wallet_transactions", |s: StoredMarketEntry| MarketEntry::from(s));
    Self {
      assets,
      contracts,
      mail,
      skills,
      wallet_journal,
      wallet_transactions,
    }
  }

  /// Returns the asset records for the given character.
  pub fn assets_for(&self, character_id: i64) -> Vec<AssetRecord> {
    self.assets.get(&character_id).cloned().unwrap_or_default()
  }

  /// Returns contracts for the given character.
  pub fn contracts_for(&self, character_id: i64) -> Vec<ContractEntry> {
    self.contracts.get(&character_id).cloned().unwrap_or_default()
  }

  /// Returns cached mail messages for the given character.
  pub fn mail_for(&self, character_id: i64) -> Vec<MailMessage> {
    self.mail.get(&character_id).cloned().unwrap_or_default()
  }

  /// Replaces the asset records for `character_id` and flushes to disk.
  pub fn set_assets(&mut self, character_id: i64, records: Vec<AssetRecord>) {
    let stored: Vec<StoredAssetRecord> = records.iter().map(StoredAssetRecord::from_view).collect();
    persist("assets", character_id, &stored);
    self.assets.insert(character_id, records);
  }

  /// Replaces contracts for `character_id` and flushes to disk.
  pub fn set_contracts(&mut self, character_id: i64, entries: Vec<ContractEntry>) {
    let stored: Vec<StoredContractEntry> = entries.iter().map(StoredContractEntry::from_view).collect();
    persist("contracts", character_id, &stored);
    self.contracts.insert(character_id, entries);
  }

  /// Replaces mail messages for `character_id` and flushes to disk.
  pub fn set_mail(&mut self, character_id: i64, messages: Vec<MailMessage>) {
    let stored: Vec<StoredMailMessage> = messages.iter().map(StoredMailMessage::from_view).collect();
    persist("mail", character_id, &stored);
    self.mail.insert(character_id, messages);
  }

  /// Replaces skill group definitions for `character_id` and flushes to disk.
  pub fn set_skills(&mut self, character_id: i64, groups: Vec<pod_model::SkillGroupDef>) {
    let stored: Vec<StoredSkillGroupDef> = groups.iter().map(StoredSkillGroupDef::from_model).collect();
    persist("skills", character_id, &stored);
    self.skills.insert(character_id, groups);
  }

  /// Replaces wallet journal entries for `character_id` and flushes to disk.
  pub fn set_wallet_journal(&mut self, character_id: i64, entries: Vec<JournalEntry>) {
    let stored: Vec<StoredJournalEntry> = entries.iter().map(StoredJournalEntry::from_view).collect();
    persist("wallet_journal", character_id, &stored);
    self.wallet_journal.insert(character_id, entries);
  }

  /// Replaces wallet transactions for `character_id` and flushes to disk.
  pub fn set_wallet_transactions(&mut self, character_id: i64, entries: Vec<MarketEntry>) {
    let stored: Vec<StoredMarketEntry> = entries.iter().map(StoredMarketEntry::from_view).collect();
    persist("wallet_transactions", character_id, &stored);
    self.wallet_transactions.insert(character_id, entries);
  }

  /// Returns skill group definitions for the given character.
  pub fn skills_for(&self, character_id: i64) -> Vec<pod_model::SkillGroupDef> {
    self.skills.get(&character_id).cloned().unwrap_or_default()
  }

  /// Returns wallet journal entries for the given character.
  pub fn wallet_journal_for(&self, character_id: i64) -> Vec<JournalEntry> {
    self.wallet_journal.get(&character_id).cloned().unwrap_or_default()
  }

  /// Returns wallet market transactions for the given character.
  pub fn wallet_transactions_for(&self, character_id: i64) -> Vec<MarketEntry> {
    self.wallet_transactions.get(&character_id).cloned().unwrap_or_default()
  }
}

/// Returns the base directory for all DataStore persistence files.
///
/// Resolves to `{state_home}/pod`.
fn state_dir() -> PathBuf {
  dir_spec::state_home()
    .map(|p| p.join("pod"))
    .expect("failed to resolve state home directory")
}

/// Returns the path for a single character's data file of the given type.
fn data_path(data_type: &str, character_id: i64) -> PathBuf {
  state_dir().join(data_type).join(format!("{character_id}.bin"))
}

/// Reads all persisted files for `data_type`, deserializes each as
/// `Vec<S>`, maps each element with `convert`, and returns a
/// `HashMap<character_id, Vec<T>>`. Errors are logged and treated as empty.
fn load_map<S, T, F>(data_type: &str, convert: F) -> HashMap<i64, Vec<T>>
where
  S: for<'de> Deserialize<'de>,
  F: Fn(S) -> T,
{
  let dir = state_dir().join(data_type);
  let entries = match std::fs::read_dir(&dir) {
    Ok(rd) => rd,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return HashMap::new(),
    Err(e) => {
      tracing::warn!("data_store: failed to read {data_type} directory: {e}");
      return HashMap::new();
    }
  };
  let mut map = HashMap::new();
  for entry in entries.flatten() {
    load_file(&entry.path(), data_type, &convert, &mut map);
  }
  map
}

/// Loads a single character data file into `map`, logging any parse errors.
fn load_file<S, T, F>(path: &std::path::Path, data_type: &str, convert: &F, map: &mut HashMap<i64, Vec<T>>)
where
  S: for<'de> Deserialize<'de>,
  F: Fn(S) -> T,
{
  let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
    return;
  };
  let Ok(character_id) = stem.parse::<i64>() else {
    return;
  };
  let bytes = match std::fs::read(path) {
    Ok(b) => b,
    Err(e) => {
      tracing::warn!("data_store: {data_type}/{character_id}.bin: read error — {e}");
      return;
    }
  };
  match serde_json::from_slice::<Vec<S>>(&bytes) {
    Ok(records) => {
      map.insert(character_id, records.into_iter().map(convert).collect());
    }
    Err(e) => tracing::warn!("data_store: {data_type}/{character_id}.bin: parse error — {e}"),
  }
}

/// Serializes `records` to `{state_dir}/{data_type}/{character_id}.bin`.
///
/// Creates parent directories as needed. Errors are logged, never panicked.
fn persist<T: Serialize>(data_type: &str, character_id: i64, records: &T) {
  let path = data_path(data_type, character_id);
  if let Some(parent) = path.parent()
    && let Err(e) = std::fs::create_dir_all(parent) {
      tracing::warn!("data_store: failed to create {data_type} directory: {e}");
      return;
    }
  match serde_json::to_vec(records) {
    Ok(bytes) => {
      if let Err(e) = std::fs::write(&path, bytes) {
        tracing::warn!("data_store: failed to write {data_type}/{character_id}.bin: {e}");
      }
    }
    Err(e) => tracing::warn!("data_store: failed to serialize {data_type}/{character_id}: {e}"),
  }
}

/// Serializable stored form of `pod_ui::views::assets::AssetRecord`.
#[derive(Deserialize, Serialize)]
struct StoredAssetRecord {
  category_key: String,
  character_id: i64,
  container_id: i64,
  container_path: String,
  constellation_id: i32,
  constellation_name: String,
  depth: usize,
  group_name: String,
  icon_variant: String,
  is_container: bool,
  is_singleton: bool,
  item_id: i64,
  location_id: i64,
  location_name: String,
  quantity: i64,
  region_id: i32,
  region_name: String,
  system_name: String,
  type_id: i32,
  type_name: String,
  unit_price: f64,
  volume: f64,
}

impl StoredAssetRecord {
  fn from_view(r: &AssetRecord) -> Self {
    Self {
      category_key: r.category_key.clone(),
      character_id: r.character_id,
      container_id: r.container_id,
      container_path: r.container_path.clone(),
      constellation_id: r.constellation_id,
      constellation_name: r.constellation_name.clone(),
      depth: r.depth,
      group_name: r.group_name.clone(),
      icon_variant: r.icon_variant.clone(),
      is_container: r.is_container,
      is_singleton: r.is_singleton,
      item_id: r.item_id,
      location_id: r.location_id,
      location_name: r.location_name.clone(),
      quantity: r.quantity,
      region_id: r.region_id,
      region_name: r.region_name.clone(),
      system_name: r.system_name.clone(),
      type_id: r.type_id,
      type_name: r.type_name.clone(),
      unit_price: r.unit_price,
      volume: r.volume,
    }
  }
}

impl From<StoredAssetRecord> for AssetRecord {
  fn from(s: StoredAssetRecord) -> Self {
    Self {
      category_key: s.category_key,
      character_id: s.character_id,
      container_id: s.container_id,
      container_path: s.container_path,
      constellation_id: s.constellation_id,
      constellation_name: s.constellation_name,
      depth: s.depth,
      group_name: s.group_name,
      icon_variant: s.icon_variant,
      is_container: s.is_container,
      is_singleton: s.is_singleton,
      item_id: s.item_id,
      location_id: s.location_id,
      location_name: s.location_name,
      quantity: s.quantity,
      region_id: s.region_id,
      region_name: s.region_name,
      system_name: s.system_name,
      type_id: s.type_id,
      type_name: s.type_name,
      unit_price: s.unit_price,
      volume: s.volume,
    }
  }
}

/// Serializable stored form of `pod_ui::views::wallet::ContractEntry`.
#[derive(Deserialize, Serialize)]
struct StoredContractEntry {
  collateral: f64,
  counterparty: String,
  id: String,
  kind: String,
  location: String,
  price: f64,
  status: String,
  title: String,
  ts_secs: u64,
  who: i64,
}

impl StoredContractEntry {
  fn from_view(e: &ContractEntry) -> Self {
    Self {
      collateral: e.collateral,
      counterparty: e.counterparty.clone(),
      id: e.id.clone(),
      kind: e.kind.clone(),
      location: e.location.clone(),
      price: e.price,
      status: e.status.clone(),
      title: e.title.clone(),
      ts_secs: e.ts_secs,
      who: e.who,
    }
  }
}

impl From<StoredContractEntry> for ContractEntry {
  fn from(s: StoredContractEntry) -> Self {
    Self {
      collateral: s.collateral,
      counterparty: s.counterparty,
      id: s.id,
      kind: s.kind,
      location: s.location,
      price: s.price,
      status: s.status,
      title: s.title,
      ts_secs: s.ts_secs,
      who: s.who,
    }
  }
}

/// Serializable stored form of `pod_ui::views::wallet::JournalEntry`.
#[derive(Deserialize, Serialize)]
struct StoredJournalEntry {
  delta: f64,
  entry_type: String,
  id: String,
  location: String,
  party: String,
  reference: String,
  ts_secs: u64,
  who: i64,
}

impl StoredJournalEntry {
  fn from_view(e: &JournalEntry) -> Self {
    Self {
      delta: e.delta,
      entry_type: e.entry_type.clone(),
      id: e.id.clone(),
      location: e.location.clone(),
      party: e.party.clone(),
      reference: e.reference.clone(),
      ts_secs: e.ts_secs,
      who: e.who,
    }
  }
}

impl From<StoredJournalEntry> for JournalEntry {
  fn from(s: StoredJournalEntry) -> Self {
    Self {
      delta: s.delta,
      entry_type: s.entry_type,
      id: s.id,
      location: s.location,
      party: s.party,
      reference: s.reference,
      ts_secs: s.ts_secs,
      who: s.who,
    }
  }
}

/// Serializable stored form of `pod_ui::views::mail::MailMessage`.
#[derive(Deserialize, Serialize)]
struct StoredMailMessage {
  body: Vec<String>,
  body_loaded: bool,
  character_id: i64,
  date_label: String,
  folder: String,
  from_corp: bool,
  from_id: Option<i64>,
  from_name: String,
  from_system: bool,
  from_tone: u16,
  has_attachment: bool,
  id: String,
  important: bool,
  labels: Vec<String>,
  mail_id: i64,
  pinned: bool,
  preview: String,
  recipients_display: String,
  snoozed: Option<String>,
  starred: bool,
  subject: String,
  time: String,
  unread: bool,
}

impl StoredMailMessage {
  fn from_view(m: &MailMessage) -> Self {
    Self {
      body: m.body.clone(),
      body_loaded: m.body_loaded,
      character_id: m.character_id,
      date_label: m.date_label.clone(),
      folder: m.folder.clone(),
      from_corp: m.from_corp,
      from_id: m.from_id,
      from_name: m.from_name.clone(),
      from_system: m.from_system,
      from_tone: m.from_tone,
      has_attachment: m.has_attachment,
      id: m.id.clone(),
      important: m.important,
      labels: m.labels.clone(),
      mail_id: m.mail_id,
      pinned: m.pinned,
      preview: m.preview.clone(),
      recipients_display: m.recipients_display.clone(),
      snoozed: m.snoozed.clone(),
      starred: m.starred,
      subject: m.subject.clone(),
      time: m.time.clone(),
      unread: m.unread,
    }
  }
}

impl From<StoredMailMessage> for MailMessage {
  fn from(s: StoredMailMessage) -> Self {
    Self {
      body: s.body,
      body_loaded: s.body_loaded,
      character_id: s.character_id,
      date_label: s.date_label,
      folder: s.folder,
      from_corp: s.from_corp,
      from_id: s.from_id,
      from_name: s.from_name,
      from_system: s.from_system,
      from_tone: s.from_tone,
      has_attachment: s.has_attachment,
      id: s.id,
      important: s.important,
      labels: s.labels,
      mail_id: s.mail_id,
      pinned: s.pinned,
      preview: s.preview,
      recipients_display: s.recipients_display,
      snoozed: s.snoozed,
      starred: s.starred,
      subject: s.subject,
      time: s.time,
      unread: s.unread,
    }
  }
}

/// Serializable stored form of `pod_ui::views::wallet::MarketEntry`.
#[derive(Deserialize, Serialize)]
struct StoredMarketEntry {
  fee: f64,
  id: String,
  item: String,
  location: String,
  qty: u64,
  side: String,
  total: f64,
  ts_secs: u64,
  type_id: i32,
  unit: f64,
  who: i64,
}

impl StoredMarketEntry {
  fn from_view(e: &MarketEntry) -> Self {
    Self {
      fee: e.fee,
      id: e.id.clone(),
      item: e.item.clone(),
      location: e.location.clone(),
      qty: e.qty,
      side: e.side.clone(),
      total: e.total,
      ts_secs: e.ts_secs,
      type_id: e.type_id,
      unit: e.unit,
      who: e.who,
    }
  }
}

impl From<StoredMarketEntry> for MarketEntry {
  fn from(s: StoredMarketEntry) -> Self {
    Self {
      fee: s.fee,
      id: s.id,
      item: s.item,
      location: s.location,
      qty: s.qty,
      side: s.side,
      total: s.total,
      ts_secs: s.ts_secs,
      type_id: s.type_id,
      unit: s.unit,
      who: s.who,
    }
  }
}

/// Serializable stored form of `pod_model::SkillGroupDef`.
#[derive(Deserialize, Serialize)]
struct StoredSkillGroupDef {
  id: String,
  name: String,
  skills: Vec<StoredSkillDef>,
}

impl StoredSkillGroupDef {
  fn from_model(g: &pod_model::SkillGroupDef) -> Self {
    Self {
      id: g.id.clone(),
      name: g.name.clone(),
      skills: g.skills.iter().map(StoredSkillDef::from_model).collect(),
    }
  }

  fn into_model(self) -> pod_model::SkillGroupDef {
    pod_model::SkillGroupDef {
      id: self.id,
      name: self.name,
      skills: self.skills.into_iter().map(StoredSkillDef::into_model).collect(),
    }
  }
}

/// Serializable stored form of `pod_model::SkillDef`.
#[derive(Deserialize, Serialize)]
struct StoredSkillDef {
  level: u8,
  name: String,
  prereqs: Vec<(String, u8)>,
  primary: StoredAttrKey,
  rank: u8,
  secondary: StoredAttrKey,
  sp: u64,
  type_id: i32,
}

impl StoredSkillDef {
  fn from_model(s: &pod_model::SkillDef) -> Self {
    Self {
      level: s.level,
      name: s.name.clone(),
      prereqs: s.prereqs.clone(),
      primary: StoredAttrKey::from(s.primary),
      rank: s.rank,
      secondary: StoredAttrKey::from(s.secondary),
      sp: s.sp,
      type_id: s.type_id,
    }
  }

  fn into_model(self) -> pod_model::SkillDef {
    pod_model::SkillDef {
      level: self.level,
      name: self.name,
      prereqs: self.prereqs,
      primary: AttrKey::from(self.primary),
      rank: self.rank,
      secondary: AttrKey::from(self.secondary),
      sp: self.sp,
      type_id: self.type_id,
    }
  }
}

/// Serializable stored form of `pod_model::AttrKey`.
#[derive(Deserialize, Serialize)]
enum StoredAttrKey {
  Charisma,
  Intelligence,
  Memory,
  Perception,
  Willpower,
}

impl From<AttrKey> for StoredAttrKey {
  fn from(k: AttrKey) -> Self {
    match k {
      AttrKey::Charisma => Self::Charisma,
      AttrKey::Intelligence => Self::Intelligence,
      AttrKey::Memory => Self::Memory,
      AttrKey::Perception => Self::Perception,
      AttrKey::Willpower => Self::Willpower,
    }
  }
}

impl From<StoredAttrKey> for AttrKey {
  fn from(k: StoredAttrKey) -> Self {
    match k {
      StoredAttrKey::Charisma => Self::Charisma,
      StoredAttrKey::Intelligence => Self::Intelligence,
      StoredAttrKey::Memory => Self::Memory,
      StoredAttrKey::Perception => Self::Perception,
      StoredAttrKey::Willpower => Self::Willpower,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod data_store {
    use super::*;

    mod load {
      use super::*;

      #[test]
      fn it_does_not_panic_when_state_directory_is_absent_or_populated() {
        let _store = DataStore::load();
      }
    }

    mod assets_for {
      use super::*;

      #[test]
      fn it_returns_empty_vec_for_unknown_character() {
        let store = DataStore::load();

        assert!(store.assets_for(12345).is_empty());
      }
    }

    mod contracts_for {
      use super::*;

      #[test]
      fn it_returns_empty_vec_for_unknown_character() {
        let store = DataStore::load();

        assert!(store.contracts_for(12345).is_empty());
      }
    }

    mod mail_for {
      use super::*;

      #[test]
      fn it_returns_empty_vec_for_unknown_character() {
        let store = DataStore::load();

        assert!(store.mail_for(12345).is_empty());
      }
    }

    mod skills_for {
      use super::*;

      #[test]
      fn it_returns_empty_vec_for_unknown_character() {
        let store = DataStore::load();

        assert!(store.skills_for(12345).is_empty());
      }
    }

    mod wallet_journal_for {
      use super::*;

      #[test]
      fn it_returns_empty_vec_for_unknown_character() {
        let store = DataStore::load();

        assert!(store.wallet_journal_for(12345).is_empty());
      }
    }

    mod wallet_transactions_for {
      use super::*;

      #[test]
      fn it_returns_empty_vec_for_unknown_character() {
        let store = DataStore::load();

        assert!(store.wallet_transactions_for(12345).is_empty());
      }
    }

    mod set_assets {
      use super::*;

      #[test]
      fn it_updates_in_memory_cache() {
        let mut store = DataStore::load();
        let record = make_asset_record(99);
        store.set_assets(99, vec![record]);

        assert_eq!(store.assets_for(99).len(), 1);
      }
    }

    mod set_contracts {
      use super::*;

      #[test]
      fn it_updates_in_memory_cache() {
        let mut store = DataStore::load();
        let entry = make_contract_entry(99);
        store.set_contracts(99, vec![entry]);

        assert_eq!(store.contracts_for(99).len(), 1);
      }
    }

    mod set_mail {
      use super::*;

      #[test]
      fn it_updates_in_memory_cache() {
        let mut store = DataStore::load();
        let msg = make_mail_message(99);
        store.set_mail(99, vec![msg]);

        assert_eq!(store.mail_for(99).len(), 1);
      }
    }

    mod set_skills {
      use super::*;

      #[test]
      fn it_updates_in_memory_cache() {
        let mut store = DataStore::load();
        let group = make_skill_group();
        store.set_skills(99, vec![group]);

        assert_eq!(store.skills_for(99).len(), 1);
      }
    }

    mod set_wallet_journal {
      use super::*;

      #[test]
      fn it_updates_in_memory_cache() {
        let mut store = DataStore::load();
        let entry = make_journal_entry(99);
        store.set_wallet_journal(99, vec![entry]);

        assert_eq!(store.wallet_journal_for(99).len(), 1);
      }
    }

    mod set_wallet_transactions {
      use super::*;

      #[test]
      fn it_updates_in_memory_cache() {
        let mut store = DataStore::load();
        let entry = make_market_entry(99);
        store.set_wallet_transactions(99, vec![entry]);

        assert_eq!(store.wallet_transactions_for(99).len(), 1);
      }
    }
  }

  mod stored_attr_key {
    use super::*;

    mod round_trip {
      use super::*;

      #[test]
      fn it_round_trips_all_variants() {
        for key in AttrKey::ALL {
          let stored = StoredAttrKey::from(key);
          let back = AttrKey::from(stored);
          assert_eq!(back, key);
        }
      }
    }
  }

  fn make_asset_record(character_id: i64) -> AssetRecord {
    AssetRecord {
      category_key: "ship".into(),
      character_id,
      container_id: 0,
      container_path: String::new(),
      constellation_id: 0,
      constellation_name: String::new(),
      depth: 0,
      group_name: "Frigate".into(),
      icon_variant: "icon".into(),
      is_container: false,
      is_singleton: true,
      item_id: 1,
      location_id: 60_003_760,
      location_name: "Jita IV".into(),
      quantity: 1,
      region_id: 10_000_002,
      region_name: "The Forge".into(),
      system_name: "Jita".into(),
      type_id: 587,
      type_name: "Rifter".into(),
      unit_price: 0.0,
      volume: 27_289.0,
    }
  }

  fn make_contract_entry(who: i64) -> ContractEntry {
    ContractEntry {
      collateral: 0.0,
      counterparty: "Test Corp".into(),
      id: format!("{who}-1"),
      kind: "item_exchange".into(),
      location: "Jita IV".into(),
      price: 1_000_000.0,
      status: "outstanding".into(),
      title: "Rifter x10".into(),
      ts_secs: 0,
      who,
    }
  }

  fn make_journal_entry(who: i64) -> JournalEntry {
    JournalEntry {
      delta: 1_000_000.0,
      entry_type: "player_donation".into(),
      id: format!("{who}-1"),
      location: String::new(),
      party: "Someone".into(),
      reference: String::new(),
      ts_secs: 0,
      who,
    }
  }

  fn make_mail_message(character_id: i64) -> MailMessage {
    MailMessage {
      body: Vec::new(),
      body_loaded: false,
      character_id,
      date_label: "Today".into(),
      folder: "Inbox".into(),
      from_corp: false,
      from_id: None,
      from_name: "Pilot A".into(),
      from_system: false,
      from_tone: 0,
      has_attachment: false,
      id: format!("{character_id}-1"),
      important: false,
      labels: Vec::new(),
      mail_id: 1,
      pinned: false,
      preview: "Hello".into(),
      recipients_display: String::new(),
      snoozed: None,
      starred: false,
      subject: "Test".into(),
      time: "12:00".into(),
      unread: true,
    }
  }

  fn make_market_entry(who: i64) -> MarketEntry {
    MarketEntry {
      fee: 0.0,
      id: format!("{who}-1"),
      item: "Rifter".into(),
      location: "Jita IV".into(),
      qty: 10,
      side: "sell".into(),
      total: 10_000_000.0,
      ts_secs: 0,
      type_id: 587,
      unit: 1_000_000.0,
      who,
    }
  }

  fn make_skill_group() -> pod_model::SkillGroupDef {
    pod_model::SkillGroupDef {
      id: "spaceship_command".into(),
      name: "Spaceship Command".into(),
      skills: vec![],
    }
  }
}
