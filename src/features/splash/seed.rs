use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

use iced::{Task, futures::SinkExt as _};
use serde::Deserialize;

use crate::{
  clients::{esi::models::universe::DogmaAttribute, http, sde as sde_client},
  store::{
    Database,
    model::{
      AbyssalModuleStat, Bloodline, Certificate, CertificateSkill, DogmaAttribute as DogmaAttributeMeta, Faction,
      ItemCategory, ItemGroup, ItemType, MarketGroup, Race, ShipMastery, SkillMetadata,
    },
    repo::{assets, sde, skills},
  },
};

const SEED_FORMAT_REVISION: u32 = 2;

const SKILL_CATEGORY_ID: i64 = 16;
const SKILL_RANK_ATTR_ID: i32 = 275;
const SKILL_PRIMARY_ATTR_ID: i32 = 180;
const SKILL_SECONDARY_ATTR_ID: i32 = 181;

#[derive(Clone, Debug)]
pub enum Progress {
  Complete,
  Error(String),
  Step(String),
}

type Tx = iced::futures::channel::mpsc::Sender<Progress>;

#[derive(Clone, Deserialize)]
struct LocalizedString {
  en: Option<String>,
}

impl LocalizedString {
  fn en(self) -> String {
    self.en.unwrap_or_default()
  }
}

#[derive(Deserialize)]
struct SdeCategoryEntry {
  name: LocalizedString,
  #[serde(rename = "iconID")]
  icon_id: Option<i64>,
  #[serde(default = "default_true")]
  published: bool,
}

#[derive(Deserialize)]
struct SdeGroupEntry {
  #[serde(rename = "categoryID")]
  category_id: i64,
  name: LocalizedString,
  #[serde(rename = "iconID")]
  icon_id: Option<i64>,
  #[serde(default = "default_true")]
  published: bool,
}

#[derive(Deserialize)]
struct SdeMarketGroupEntry {
  name: Option<LocalizedString>,
  description: Option<LocalizedString>,
  #[serde(rename = "hasTypes")]
  has_types: Option<bool>,
  #[serde(rename = "iconID")]
  icon_id: Option<i64>,
  #[serde(rename = "parentGroupID")]
  parent_group_id: Option<i64>,
}

#[derive(Deserialize)]
struct SdeTypeEntry {
  name: LocalizedString,
  description: Option<LocalizedString>,
  #[serde(rename = "groupID")]
  group_id: i64,
  #[serde(rename = "marketGroupID")]
  market_group_id: Option<i64>,
  capacity: Option<f64>,
  volume: Option<f64>,
  #[serde(rename = "packagedVolume")]
  packaged_volume: Option<f64>,
  #[serde(rename = "portionSize")]
  portion_size: Option<i32>,
  radius: Option<f64>,
  #[serde(default = "default_true")]
  published: bool,
  #[serde(rename = "iconID")]
  icon_id: Option<i64>,
}

#[derive(Default, Deserialize)]
struct SdeTypeDogmaEntry {
  #[serde(rename = "dogmaAttributes", default)]
  dogma_attributes: Vec<SdeDogmaAttribute>,
}

#[derive(Deserialize)]
struct SdeDogmaAttribute {
  #[serde(rename = "attributeID")]
  attribute_id: i32,
  value: f64,
}

#[derive(Deserialize)]
struct SdeDogmaAttrEntry {
  name: String,
  #[serde(rename = "displayName")]
  display_name: Option<LocalizedString>,
  #[serde(default)]
  description: Option<String>,
  #[serde(rename = "defaultValue")]
  default_value: Option<f64>,
  #[serde(rename = "highIsGood", default)]
  high_is_good: bool,
  #[serde(rename = "iconID")]
  icon_id: Option<i64>,
  #[serde(default)]
  published: bool,
  #[serde(default = "default_true")]
  stackable: bool,
  #[serde(rename = "unitID")]
  unit_id: Option<i64>,
}

#[derive(Deserialize)]
struct SdeDynamicEntry {
  #[serde(rename = "attributeIDs", default)]
  attribute_ids: HashMap<i32, SdeDynamicAttrBounds>,
  #[serde(rename = "inputOutputMapping", default)]
  input_output_mapping: Vec<SdeDynamicMapping>,
}

#[derive(Deserialize)]
struct SdeDynamicAttrBounds {
  min: f64,
  max: f64,
}

#[derive(Deserialize)]
struct SdeDynamicMapping {
  #[serde(rename = "resultingType")]
  resulting_type: i32,
}

#[derive(Deserialize)]
struct SdeRaceEntry {
  name: Option<LocalizedString>,
  #[serde(rename = "allianceID")]
  alliance_id: Option<i32>,
}

#[derive(Deserialize)]
struct SdeBloodlineEntry {
  name: Option<LocalizedString>,
  #[serde(rename = "raceID", default)]
  race_id: i32,
  #[serde(rename = "corporationID", default)]
  corporation_id: i32,
  #[serde(rename = "shipTypeID", default)]
  ship_type_id: i32,
  #[serde(default)]
  charisma: i32,
  #[serde(default)]
  intelligence: i32,
  #[serde(default)]
  memory: i32,
  #[serde(default)]
  perception: i32,
  #[serde(default)]
  willpower: i32,
}

#[derive(Deserialize)]
struct SdeFactionEntry {
  name: Option<LocalizedString>,
  #[serde(rename = "sizeFactor", default = "default_one_f64")]
  size_factor: f64,
  #[serde(rename = "solarSystemID")]
  solar_system_id: Option<i32>,
  #[serde(rename = "isUnique")]
  is_unique: Option<bool>,
}

#[derive(Deserialize)]
struct CertSkillLevel {
  #[serde(default)]
  basic: i32,
  #[serde(default)]
  improved: i32,
  #[serde(default)]
  advanced: i32,
  #[serde(default)]
  elite: i32,
}

#[derive(Deserialize)]
struct SdeCertEntry {
  name: LocalizedString,
  #[serde(default)]
  description: Option<LocalizedString>,
  #[serde(default)]
  grade: Option<i32>,
  #[serde(rename = "skillTypes", default)]
  skill_types: HashMap<i32, CertSkillLevel>,
}

pub fn seed(db: Database, http: Arc<http::Client>) -> Task<Progress> {
  let (tx, rx) = iced::futures::channel::mpsc::channel(64);
  tokio::spawn(run_seed(db, http, tx));
  Task::stream(rx)
}

async fn run_seed(db: Database, http: Arc<http::Client>, mut tx: Tx) {
  match do_seed(&db, http, &mut tx).await {
    Ok(()) => {
      let _ = tx.send(Progress::Complete).await;
    }
    Err(e) => {
      let _ = tx.send(Progress::Error(format!("SDE seed error: {e}"))).await;
    }
  }
}

async fn do_seed(db: &Database, http: Arc<http::Client>, tx: &mut Tx) -> Result<(), String> {
  step(tx, "Downloading static data\u{2026}").await;
  let extracted = sde_client::Client::new(http)
    .download_and_extract()
    .await
    .map_err(|e| e.to_string())?;

  seed_if_stale(db, tx, &extracted.root, extracted.build_version.as_deref()).await
}

async fn seed_if_stale(db: &Database, tx: &mut Tx, root: &Path, build_version: Option<&str>) -> Result<(), String> {
  seed_if_stale_at(db, tx, root, build_version, sde_version_path().as_deref()).await
}

async fn seed_if_stale_at(
  db: &Database,
  tx: &mut Tx,
  root: &Path,
  build_version: Option<&str>,
  marker_path: Option<&Path>,
) -> Result<(), String> {
  let composite = build_version.map(composite_version);
  if sde_is_current(marker_path, composite.as_deref()) {
    backfill_dogma_attributes(db, tx, root).await?;
    return Ok(());
  }

  seed_all_tables(db, tx, root).await?;

  if let (Some(path), Some(version)) = (marker_path, composite.as_deref()) {
    write_stored_sde_version(path, version);
  }

  Ok(())
}

async fn backfill_dogma_attributes(db: &Database, tx: &mut Tx, root: &Path) -> Result<(), String> {
  if sde::is_seeded(db).await.map_err(|e| e.to_string())? {
    return Ok(());
  }

  let path = root.join("dogmaAttributes.yaml");
  if path.exists() {
    step(tx, "Backfilling dogma attributes\u{2026}").await;
    seed_dogma_attributes(db, &path).await?;
  }

  Ok(())
}

fn sde_is_current(marker_path: Option<&Path>, composite: Option<&str>) -> bool {
  let (Some(marker_path), Some(composite)) = (marker_path, composite) else {
    return false;
  };
  read_stored_sde_version(marker_path).as_deref() == Some(composite)
}

async fn seed_all_tables(db: &Database, tx: &mut Tx, r: &Path) -> Result<(), String> {
  step(tx, "Seeding item categories\u{2026}").await;
  seed_categories(db, &r.join("categories.yaml")).await?;

  step(tx, "Seeding item groups\u{2026}").await;
  seed_groups(db, &r.join("groups.yaml")).await?;

  step(tx, "Seeding market groups\u{2026}").await;
  seed_market_groups(db, &r.join("marketGroups.yaml")).await?;

  step(tx, "Seeding item types\u{2026}").await;
  seed_types(
    db,
    &r.join("types.yaml"),
    &r.join("typeDogma.yaml"),
    &r.join("groups.yaml"),
  )
  .await?;

  step(tx, "Seeding dogma attributes\u{2026}").await;
  seed_dogma_attributes(db, &r.join("dogmaAttributes.yaml")).await?;

  let dynamic_path = r.join("dynamicItemAttributes.yaml");
  if dynamic_path.exists() {
    step(tx, "Seeding abyssal module stats\u{2026}").await;
    seed_abyssal_module_stats(db, &dynamic_path).await?;
  }

  step(tx, "Seeding races\u{2026}").await;
  seed_races(db, &r.join("races.yaml")).await?;

  step(tx, "Seeding bloodlines\u{2026}").await;
  seed_bloodlines(db, &r.join("bloodlines.yaml")).await?;

  step(tx, "Seeding factions\u{2026}").await;
  seed_factions(db, &r.join("factions.yaml")).await?;

  let cert_path = r.join("certificates.yaml");
  if cert_path.exists() {
    step(tx, "Seeding certificates\u{2026}").await;
    seed_certificates(db, &cert_path).await?;
  }

  let mastery_path = r.join("masteries.yaml");
  if mastery_path.exists() {
    step(tx, "Seeding ship masteries\u{2026}").await;
    seed_masteries(db, &mastery_path).await?;
  }

  Ok(())
}

async fn step(tx: &mut Tx, label: &str) {
  let _ = tx.send(Progress::Step(label.to_string())).await;
}

async fn read_yaml<T: serde::de::DeserializeOwned + Send + 'static>(path: &Path) -> Result<T, String> {
  let path = path.to_owned();
  tokio::task::spawn_blocking(move || {
    let data = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_yaml::from_slice::<T>(&data).map_err(|e| format!("parse {}: {e}", path.display()))
  })
  .await
  .map_err(|e| e.to_string())?
}

async fn seed_categories(db: &Database, path: &Path) -> Result<(), String> {
  let entries: HashMap<i64, SdeCategoryEntry> = read_yaml(path).await?;

  let records: Vec<ItemCategory> = entries
    .into_iter()
    .map(|(id, e)| ItemCategory {
      icon_id: e.icon_id,
      id,
      name: e.name.en(),
      published: e.published,
    })
    .collect();

  sde::upsert_many_item_categories(db, &records)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_groups(db: &Database, path: &Path) -> Result<(), String> {
  let entries: HashMap<i64, SdeGroupEntry> = read_yaml(path).await?;

  let records: Vec<ItemGroup> = entries
    .into_iter()
    .map(|(id, e)| ItemGroup {
      category_id: e.category_id,
      icon_id: e.icon_id,
      id,
      name: e.name.en(),
      published: e.published,
    })
    .collect();

  sde::upsert_many_item_groups(db, &records)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_market_groups(db: &Database, path: &Path) -> Result<(), String> {
  let entries: HashMap<i64, SdeMarketGroupEntry> = read_yaml(path).await?;

  let records: Vec<MarketGroup> = entries
    .into_iter()
    .map(|(id, e)| MarketGroup {
      description: e.description.map(LocalizedString::en).unwrap_or_default(),
      has_types: e.has_types.unwrap_or(false),
      icon_id: e.icon_id,
      id,
      name: e.name.map(LocalizedString::en).unwrap_or_default(),
      parent_id: e.parent_group_id,
    })
    .collect();

  sde::upsert_many_market_groups(db, &records)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_types(db: &Database, types_path: &Path, dogma_path: &Path, groups_path: &Path) -> Result<(), String> {
  let entries: HashMap<i64, SdeTypeEntry> = read_yaml(types_path).await?;
  let dogma: HashMap<i64, SdeTypeDogmaEntry> = read_yaml(dogma_path).await?;
  let groups: HashMap<i64, SdeGroupEntry> = read_yaml(groups_path).await?;

  let skill_metadata: Vec<SkillMetadata> = entries
    .iter()
    .filter(|(_, e)| {
      e.published
        && groups
          .get(&e.group_id)
          .is_some_and(|g| g.category_id == SKILL_CATEGORY_ID)
    })
    .filter_map(|(&id, _)| build_skill_metadata(id, dogma.get(&id)))
    .collect();

  let records: Vec<ItemType> = entries
    .into_iter()
    .map(|(id, e)| build_item_type(id, e, dogma.get(&id)))
    .collect();

  sde::upsert_many_item_types(db, &records)
    .await
    .map_err(|e| e.to_string())?;

  for metadata in &skill_metadata {
    skills::upsert_skill_metadata(db, metadata)
      .await
      .map_err(|e| e.to_string())?;
  }
  Ok(())
}

fn build_skill_metadata(skill_id: i64, d: Option<&SdeTypeDogmaEntry>) -> Option<SkillMetadata> {
  let d = d?;
  let attr = |attribute_id: i32| {
    d.dogma_attributes
      .iter()
      .find(|a| a.attribute_id == attribute_id)
      .map(|a| a.value.round() as i64)
  };

  Some(SkillMetadata {
    primary_attribute: attr(SKILL_PRIMARY_ATTR_ID)?,
    rank: attr(SKILL_RANK_ATTR_ID)?,
    secondary_attribute: attr(SKILL_SECONDARY_ATTR_ID)?,
    skill_id,
  })
}

fn build_item_type(id: i64, e: SdeTypeEntry, d: Option<&SdeTypeDogmaEntry>) -> ItemType {
  ItemType {
    capacity: e.capacity,
    description: Some(e.description.map(LocalizedString::en).unwrap_or_default()),
    dogma_attributes: build_dogma_attributes_json(d),
    group_id: e.group_id,
    icon_id: e.icon_id,
    id,
    market_group_id: e.market_group_id,
    name: e.name.en(),
    packaged_volume: e.packaged_volume,
    portion_size: e.portion_size,
    published: e.published,
    radius: e.radius,
    volume: e.volume,
  }
}

fn build_dogma_attributes_json(d: Option<&SdeTypeDogmaEntry>) -> String {
  let attrs: Vec<DogmaAttribute> = d
    .map(|d| {
      d.dogma_attributes
        .iter()
        .map(|a| DogmaAttribute {
          attribute_id: a.attribute_id,
          value: a.value,
        })
        .collect()
    })
    .unwrap_or_default();
  serde_json::to_string(&attrs).unwrap_or_else(|_| "[]".to_owned())
}

fn build_dogma_attribute(id: i64, e: SdeDogmaAttrEntry) -> DogmaAttributeMeta {
  DogmaAttributeMeta {
    attribute_id: id,
    default_value: e.default_value,
    description: e.description.filter(|s| !s.is_empty()),
    display_name: e.display_name.map(LocalizedString::en).filter(|s| !s.is_empty()),
    high_is_good: e.high_is_good,
    icon_id: e.icon_id,
    name: e.name,
    published: e.published,
    stackable: e.stackable,
    unit_id: e.unit_id,
  }
}

async fn seed_dogma_attributes(db: &Database, path: &Path) -> Result<(), String> {
  let entries: HashMap<i64, SdeDogmaAttrEntry> = read_yaml(path).await?;

  let records: Vec<DogmaAttributeMeta> = entries
    .into_iter()
    .map(|(id, e)| build_dogma_attribute(id, e))
    .collect();

  sde::upsert_many_dogma_attributes(db, &records)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_abyssal_module_stats(db: &Database, path: &Path) -> Result<(), String> {
  let entries: HashMap<i64, SdeDynamicEntry> = read_yaml(path).await?;

  let mut records: Vec<AbyssalModuleStat> = Vec::new();
  for entry in entries.values() {
    for mapping in &entry.input_output_mapping {
      for (&attribute_id, bounds) in &entry.attribute_ids {
        records.push(AbyssalModuleStat::new(
          i64::from(mapping.resulting_type),
          i64::from(attribute_id),
          bounds.min,
          bounds.max,
        ));
      }
    }
  }

  assets::upsert_module_stats(db, &records)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_races(db: &Database, path: &Path) -> Result<(), String> {
  let entries: HashMap<i64, SdeRaceEntry> = read_yaml(path).await?;

  let records: Vec<Race> = entries
    .into_iter()
    .map(|(id, e)| {
      let name = e.name.map(LocalizedString::en).unwrap_or_default();
      Race::new(id, i64::from(e.alliance_id.unwrap_or(0)), name.clone(), name)
    })
    .collect();

  for race in &records {
    sde::upsert_race(db, race).await.map_err(|e| e.to_string())?;
  }
  Ok(())
}

async fn seed_bloodlines(db: &Database, path: &Path) -> Result<(), String> {
  let entries: HashMap<i64, SdeBloodlineEntry> = read_yaml(path).await?;

  let records: Vec<Bloodline> = entries
    .into_iter()
    .map(|(id, e)| {
      let name = e.name.map(LocalizedString::en).unwrap_or_default();
      let mut m = Bloodline::new(
        id,
        i64::from(e.corporation_id),
        i64::from(e.race_id),
        e.charisma,
        name.clone(),
        e.intelligence,
        e.memory,
        name,
        e.perception,
        e.willpower,
      );
      if e.ship_type_id != 0 {
        m.set_ship_type_id(i64::from(e.ship_type_id));
      }
      m
    })
    .collect();

  for bloodline in &records {
    sde::upsert_bloodline(db, bloodline).await.map_err(|e| e.to_string())?;
  }
  Ok(())
}

async fn seed_factions(db: &Database, path: &Path) -> Result<(), String> {
  let entries: HashMap<i64, SdeFactionEntry> = read_yaml(path).await?;

  let records: Vec<Faction> = entries
    .into_iter()
    .map(|(id, e)| {
      let name = e.name.map(LocalizedString::en).unwrap_or_default();
      let mut m = Faction::new(id, name, e.is_unique.unwrap_or(false), e.size_factor, 0, 0);
      if let Some(solar_system_id) = e.solar_system_id {
        m.set_solar_system_id(i64::from(solar_system_id));
      }
      m
    })
    .collect();

  for faction in &records {
    sde::upsert_faction(db, faction).await.map_err(|e| e.to_string())?;
  }
  Ok(())
}

async fn seed_certificates(db: &Database, path: &Path) -> Result<(), String> {
  let entries: HashMap<i64, SdeCertEntry> = read_yaml(path).await?;

  let mut certificates: Vec<Certificate> = Vec::with_capacity(entries.len());
  let mut skills: Vec<CertificateSkill> = Vec::new();

  for (id, e) in entries {
    let grade = i64::from(e.grade.unwrap_or(1).clamp(1, 5));
    for (skill_id, lvl) in &e.skill_types {
      let levels = build_cert_skill_levels(lvl);
      skills.push(CertificateSkill {
        advanced: i64::from(levels[2]),
        basic: i64::from(levels[0]),
        certificate_id: id,
        elite: i64::from(levels[3]),
        improved: i64::from(levels[1]),
        skill_id: i64::from(*skill_id),
      });
    }
    certificates.push(Certificate {
      description: e.description.map(LocalizedString::en),
      grade,
      id,
      name: e.name.en(),
    });
  }

  skills::certificate_upsert_many(db, &certificates, &skills)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_masteries(db: &Database, path: &Path) -> Result<(), String> {
  let raw_bytes = tokio::fs::read(path).await.map_err(|e| e.to_string())?;
  let raw: serde_yaml::Value =
    serde_yaml::from_slice(&raw_bytes).map_err(|e| format!("parse {}: {}", path.display(), e))?;
  let serde_yaml::Value::Mapping(outer) = raw else {
    return Ok(());
  };

  let entries = build_mastery_entries(outer);
  let records: Vec<ShipMastery> = entries
    .into_iter()
    .flat_map(|(ship_id, tier, cert_ids)| {
      cert_ids.into_iter().map(move |certificate_id| ShipMastery {
        certificate_id: i64::from(certificate_id),
        ship_type_id: i64::from(ship_id),
        tier: i64::from(tier),
      })
    })
    .collect();

  if records.is_empty() {
    return Ok(());
  }

  skills::mastery_upsert_many(db, &records)
    .await
    .map_err(|e| e.to_string())
}

fn composite_version(sde_build: &str) -> String {
  format!(
    "{}+pod-{}+seed-{}",
    sde_build,
    env!("CARGO_PKG_VERSION"),
    SEED_FORMAT_REVISION
  )
}

pub fn sde_version_path() -> Option<PathBuf> {
  Some(dir_spec::state_home()?.join("pod").join("sde_version"))
}

fn read_stored_sde_version(path: &Path) -> Option<String> {
  let contents = std::fs::read_to_string(path).ok()?;
  Some(contents.trim().to_owned())
}

fn write_stored_sde_version(path: &Path, version: &str) {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).ok();
  }
  std::fs::write(path, version).ok();
}

fn build_mastery_entries(outer: serde_yaml::Mapping) -> Vec<(i32, i32, Vec<i32>)> {
  let mut entries: Vec<(i32, i32, Vec<i32>)> = Vec::new();
  for (ship_key, tiers_val) in outer {
    let Some(ship_id) = parse_yaml_i32(&ship_key) else {
      continue;
    };
    let serde_yaml::Value::Mapping(tiers) = tiers_val else {
      continue;
    };
    for (tier_key, certs_val) in tiers {
      collect_mastery_tier(ship_id, tier_key, certs_val, &mut entries);
    }
  }
  entries
}

fn collect_mastery_tier(
  ship_id: i32,
  tier_key: serde_yaml::Value,
  certs_val: serde_yaml::Value,
  out: &mut Vec<(i32, i32, Vec<i32>)>,
) {
  let cert_ids = parse_cert_ids(certs_val);
  if cert_ids.is_empty() {
    return;
  }
  if let Some(tier_idx) = parse_tier_index(&tier_key).filter(|&t| t < 5) {
    out.push((ship_id, (tier_idx as i32) + 1, cert_ids));
  }
}

fn parse_cert_ids(certs_val: serde_yaml::Value) -> Vec<i32> {
  let serde_yaml::Value::Sequence(certs) = certs_val else {
    return Vec::new();
  };
  certs.into_iter().filter_map(|v| parse_yaml_i32(&v)).collect()
}

fn parse_tier_index(v: &serde_yaml::Value) -> Option<u8> {
  match v {
    serde_yaml::Value::Number(n) => n.as_u64().and_then(|n| u8::try_from(n).ok()),
    serde_yaml::Value::String(s) => s.parse().ok(),
    _ => None,
  }
}

fn parse_yaml_i32(v: &serde_yaml::Value) -> Option<i32> {
  match v {
    serde_yaml::Value::Number(n) => n.as_i64().and_then(|n| i32::try_from(n).ok()),
    _ => None,
  }
}

fn clamp_skill_level(v: i32) -> u8 {
  v.clamp(0, 5) as u8
}

fn build_cert_skill_levels(lvl: &CertSkillLevel) -> [u8; 4] {
  [
    clamp_skill_level(lvl.basic),
    clamp_skill_level(lvl.improved),
    clamp_skill_level(lvl.advanced),
    clamp_skill_level(lvl.elite),
  ]
}

fn default_true() -> bool {
  true
}

fn default_one_f64() -> f64 {
  1.0
}

#[cfg(test)]
mod tests {
  use super::*;

  mod build_item_type {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_type_entry(description: Option<&str>) -> SdeTypeEntry {
      SdeTypeEntry {
        name: LocalizedString {
          en: Some("Tritanium".to_owned()),
        },
        description: description.map(|d| LocalizedString {
          en: Some(d.to_owned()),
        }),
        group_id: 18,
        market_group_id: None,
        capacity: None,
        volume: None,
        packaged_volume: None,
        portion_size: None,
        radius: None,
        published: true,
        icon_id: None,
      }
    }

    #[test]
    fn it_defaults_a_missing_description_to_empty_string_never_null() {
      let model = build_item_type(34, make_type_entry(None), None);

      assert_eq!(model.description(), &Some(String::new()));
    }

    #[test]
    fn it_preserves_a_present_description() {
      let model = build_item_type(34, make_type_entry(Some("The most common ore")), None);

      assert_eq!(model.description(), &Some("The most common ore".to_owned()));
    }
  }

  mod build_dogma_attribute {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_entry(display_name: Option<&str>, description: Option<&str>) -> SdeDogmaAttrEntry {
      SdeDogmaAttrEntry {
        name: "cpuOutput".to_owned(),
        display_name: display_name.map(|d| LocalizedString {
          en: Some(d.to_owned()),
        }),
        description: description.map(str::to_owned),
        default_value: Some(0.0),
        high_is_good: true,
        icon_id: Some(1403),
        published: true,
        stackable: false,
        unit_id: Some(101),
      }
    }

    #[test]
    fn it_maps_the_localized_display_name_to_english() {
      let model = build_dogma_attribute(48, make_entry(Some("CPU Output"), None));

      assert_eq!(model.attribute_id(), 48);
      assert_eq!(model.name(), "cpuOutput");
      assert_eq!(model.display_name().as_deref(), Some("CPU Output"));
      assert_eq!(model.high_is_good(), true);
      assert_eq!(model.unit_id(), Some(101));
    }

    #[test]
    fn it_drops_empty_display_name_and_description_to_none() {
      let model = build_dogma_attribute(48, make_entry(Some(""), Some("")));

      assert_eq!(model.display_name(), &None);
      assert_eq!(model.description(), &None);
    }
  }

  mod build_skill_metadata {
    use pretty_assertions::assert_eq;

    use super::*;

    fn dogma(pairs: &[(i32, f64)]) -> SdeTypeDogmaEntry {
      SdeTypeDogmaEntry {
        dogma_attributes: pairs
          .iter()
          .map(|&(attribute_id, value)| SdeDogmaAttribute {
            attribute_id,
            value,
          })
          .collect(),
      }
    }

    #[test]
    fn it_extracts_rank_and_attribute_neural_ids_from_dogma() {
      let d = dogma(&[(275, 3.0), (180, 167.0), (181, 166.0)]);

      let result = build_skill_metadata(3300, Some(&d)).unwrap();

      assert_eq!(result.rank(), 3);
      assert_eq!(result.primary_attribute(), 167);
      assert_eq!(result.secondary_attribute(), 166);
      assert_eq!(result.skill_id(), 3300);
    }

    #[test]
    fn it_rounds_fractional_dogma_values() {
      let d = dogma(&[(275, 2.6), (180, 167.4), (181, 165.5)]);

      let result = build_skill_metadata(3300, Some(&d)).unwrap();

      assert_eq!(result.rank(), 3);
      assert_eq!(result.primary_attribute(), 167);
      assert_eq!(result.secondary_attribute(), 166);
    }

    #[test]
    fn it_returns_none_when_a_required_attribute_is_missing() {
      let d = dogma(&[(275, 1.0), (180, 167.0)]);

      assert!(build_skill_metadata(3300, Some(&d)).is_none());
    }

    #[test]
    fn it_returns_none_when_no_dogma_exists() {
      assert!(build_skill_metadata(3300, None).is_none());
    }
  }

  mod seed_types {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    async fn write_yaml(dir: &Path, name: &str, body: &str) {
      tokio::fs::write(dir.join(name), body).await.unwrap();
    }

    async fn write_fixture(dir: &Path) {
      write_yaml(
        dir,
        "categories.yaml",
        "16: { name: { en: Skill }, published: true }\n4: { name: { en: Material }, published: true }\n",
      )
      .await;
      write_yaml(
        dir,
        "groups.yaml",
        "255: { categoryID: 16, name: { en: Gunnery }, published: true }\n\
        18: { categoryID: 4, name: { en: Mineral }, published: true }\n",
      )
      .await;
      write_yaml(
        dir,
        "types.yaml",
        "3300: { groupID: 255, name: { en: Gunnery }, published: true }\n\
        3301: { groupID: 255, name: { en: Small Hybrid Turret }, published: true }\n\
        3302: { groupID: 255, name: { en: Retired Skill }, published: false }\n\
        34: { groupID: 18, name: { en: Tritanium }, published: true }\n",
      )
      .await;
      write_yaml(
        dir,
        "typeDogma.yaml",
        "3300:\n  dogmaAttributes:\n    - { attributeID: 275, value: 1.0 }\n    \
        - { attributeID: 180, value: 167.0 }\n    - { attributeID: 181, value: 166.0 }\n\
        3301:\n  dogmaAttributes:\n    - { attributeID: 275, value: 2.0 }\n    \
        - { attributeID: 180, value: 167.0 }\n    - { attributeID: 181, value: 168.0 }\n",
      )
      .await;
    }

    async fn run_seed_types(dir: &Path) -> Database {
      let db = store::open_test().await.unwrap();
      seed_categories(&db, &dir.join("categories.yaml")).await.unwrap();
      seed_groups(&db, &dir.join("groups.yaml")).await.unwrap();
      seed_types(
        &db,
        &dir.join("types.yaml"),
        &dir.join("typeDogma.yaml"),
        &dir.join("groups.yaml"),
      )
      .await
      .unwrap();
      db
    }

    #[tokio::test]
    async fn it_seeds_one_metadata_row_per_published_skill() {
      let tmp = tempfile::tempdir().unwrap();
      write_fixture(tmp.path()).await;

      let db = run_seed_types(tmp.path()).await;

      let m3300 = skills::get_skill_metadata(&db, 3300).await.unwrap().unwrap();
      assert_eq!(m3300.rank(), 1);
      assert_eq!(m3300.primary_attribute(), 167);
      assert_eq!(m3300.secondary_attribute(), 166);

      let m3301 = skills::get_skill_metadata(&db, 3301).await.unwrap().unwrap();
      assert_eq!(m3301.rank(), 2);
      assert_eq!(m3301.secondary_attribute(), 168);

      assert_eq!(skills::get_skill_metadata(&db, 34).await.unwrap(), None);
      assert_eq!(skills::get_skill_metadata(&db, 3302).await.unwrap(), None);
    }

    #[tokio::test]
    async fn it_is_idempotent_across_reseed() {
      let tmp = tempfile::tempdir().unwrap();
      write_fixture(tmp.path()).await;

      let db = run_seed_types(tmp.path()).await;
      seed_types(
        &db,
        &tmp.path().join("types.yaml"),
        &tmp.path().join("typeDogma.yaml"),
        &tmp.path().join("groups.yaml"),
      )
      .await
      .unwrap();

      let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM skill_metadata")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn it_carries_module_skill_requirement_dogma_through_to_the_picker() {
      let tmp = tempfile::tempdir().unwrap();
      write_yaml(
        tmp.path(),
        "categories.yaml",
        "7: { name: { en: Module }, published: true }\n16: { name: { en: Skill }, published: true }\n",
      )
      .await;
      write_yaml(
        tmp.path(),
        "groups.yaml",
        "55: { categoryID: 7, name: { en: Projectile Weapon }, published: true }\n\
        255: { categoryID: 16, name: { en: Gunnery }, published: true }\n",
      )
      .await;
      write_yaml(
        tmp.path(),
        "types.yaml",
        "3300: { groupID: 255, name: { en: Gunnery }, published: true }\n\
        2929: { groupID: 55, name: { en: 200mm AutoCannon I }, published: true }\n",
      )
      .await;
      write_yaml(
        tmp.path(),
        "typeDogma.yaml",
        "2929:\n  dogmaAttributes:\n    - { attributeID: 182, value: 3300.0 }\n    \
        - { attributeID: 277, value: 1.0 }\n",
      )
      .await;

      let db = run_seed_types(tmp.path()).await;

      let modules = skills::modules_for_picker(&db).await.unwrap();
      assert_eq!(modules.len(), 1);
      assert_eq!(modules[0].id, 2929);
      assert_eq!(modules[0].group_name, "Projectile Weapon");
      assert_eq!(modules[0].skill_requirements, vec![("Gunnery".to_owned(), 1)]);
    }
  }

  mod build_mastery_entries {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_outer(ship_id: i64, tier: i64, certs: Vec<i64>) -> serde_yaml::Mapping {
      let cert_seq =
        serde_yaml::Value::Sequence(certs.into_iter().map(|c| serde_yaml::Value::Number(c.into())).collect());
      let mut tier_map = serde_yaml::Mapping::new();
      tier_map.insert(serde_yaml::Value::Number(tier.into()), cert_seq);
      let mut outer = serde_yaml::Mapping::new();
      outer.insert(
        serde_yaml::Value::Number(ship_id.into()),
        serde_yaml::Value::Mapping(tier_map),
      );
      outer
    }

    #[test]
    fn it_returns_empty_for_empty_mapping() {
      let result = build_mastery_entries(serde_yaml::Mapping::new());

      assert!(result.is_empty());
    }

    #[test]
    fn it_extracts_ship_tier_and_cert_ids() {
      let outer = make_outer(1234, 2, vec![100, 200]);

      let result = build_mastery_entries(outer);

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].0, 1234);
      assert_eq!(result[0].1, 3);
      assert_eq!(result[0].2, vec![100, 200]);
    }

    #[test]
    fn it_skips_tier_index_5_and_above() {
      let outer = make_outer(1234, 5, vec![100]);

      let result = build_mastery_entries(outer);

      assert!(result.is_empty());
    }

    #[test]
    fn it_skips_entries_with_empty_cert_ids() {
      let outer = make_outer(1234, 1, vec![]);

      let result = build_mastery_entries(outer);

      assert!(result.is_empty());
    }

    #[test]
    fn it_accepts_tier_index_4_storing_it_as_5() {
      let outer = make_outer(1234, 4, vec![100]);

      let result = build_mastery_entries(outer);

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].1, 5);
    }

    #[test]
    fn it_skips_ships_with_non_mapping_tiers_value() {
      let mut outer = serde_yaml::Mapping::new();
      outer.insert(
        serde_yaml::Value::Number(1234i64.into()),
        serde_yaml::Value::String("not-a-mapping".to_string()),
      );

      let result = build_mastery_entries(outer);

      assert!(result.is_empty());
    }

    #[test]
    fn it_skips_non_numeric_ship_keys() {
      let mut outer = serde_yaml::Mapping::new();
      outer.insert(
        serde_yaml::Value::String("not-a-number".to_string()),
        serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
      );

      let result = build_mastery_entries(outer);

      assert!(result.is_empty());
    }
  }

  mod composite_version {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_embeds_the_sde_build_pod_version_and_seed_revision() {
      let result = composite_version("20240101.1");

      assert_eq!(
        result,
        format!(
          "20240101.1+pod-{}+seed-{}",
          env!("CARGO_PKG_VERSION"),
          SEED_FORMAT_REVISION
        )
      );
    }

    #[test]
    fn it_differs_when_sde_build_differs() {
      let a = composite_version("20240101.1");
      let b = composite_version("20240102.1");

      assert_ne!(a, b);
    }
  }

  mod parse_cert_ids {
    use super::*;

    #[test]
    fn it_returns_empty_for_non_sequence_value() {
      let result = parse_cert_ids(serde_yaml::Value::Null);

      assert!(result.is_empty());
    }

    #[test]
    fn it_extracts_cert_ids_from_sequence() {
      let v = serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::Number(100i64.into()),
        serde_yaml::Value::Number(200i64.into()),
      ]);

      assert_eq!(parse_cert_ids(v), vec![100, 200]);
    }

    #[test]
    fn it_skips_non_number_entries() {
      let v = serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::Number(100i64.into()),
        serde_yaml::Value::String("ignored".to_string()),
        serde_yaml::Value::Number(200i64.into()),
      ]);

      assert_eq!(parse_cert_ids(v), vec![100, 200]);
    }
  }

  mod parse_tier_index {
    use super::*;

    #[test]
    fn it_parses_numeric_tier_key() {
      let v = serde_yaml::Value::Number(2i64.into());

      assert_eq!(parse_tier_index(&v), Some(2));
    }

    #[test]
    fn it_parses_string_tier_key() {
      let v = serde_yaml::Value::String("3".to_string());

      assert_eq!(parse_tier_index(&v), Some(3));
    }

    #[test]
    fn it_returns_none_for_non_numeric_string() {
      let v = serde_yaml::Value::String("bad".to_string());

      assert_eq!(parse_tier_index(&v), None);
    }

    #[test]
    fn it_returns_none_for_mapping_value() {
      let v = serde_yaml::Value::Mapping(serde_yaml::Mapping::new());

      assert_eq!(parse_tier_index(&v), None);
    }
  }

  mod parse_yaml_i32 {
    use super::*;

    #[test]
    fn it_parses_valid_i32_from_number() {
      let v = serde_yaml::Value::Number(42i64.into());

      assert_eq!(parse_yaml_i32(&v), Some(42));
    }

    #[test]
    fn it_returns_none_for_string_value() {
      let v = serde_yaml::Value::String("42".to_string());

      assert_eq!(parse_yaml_i32(&v), None);
    }

    #[test]
    fn it_returns_none_for_value_exceeding_i32_max() {
      let large: i64 = i64::from(i32::MAX) + 1;
      let v = serde_yaml::Value::Number(large.into());

      assert_eq!(parse_yaml_i32(&v), None);
    }
  }

  mod build_cert_skill_levels {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_each_grade_level_in_order() {
      let lvl = CertSkillLevel {
        basic: 1,
        improved: 2,
        advanced: 3,
        elite: 4,
      };

      assert_eq!(build_cert_skill_levels(&lvl), [1, 2, 3, 4]);
    }

    #[test]
    fn it_clamps_levels_into_the_zero_to_five_range() {
      let lvl = CertSkillLevel {
        basic: -3,
        improved: 0,
        advanced: 5,
        elite: 9,
      };

      assert_eq!(build_cert_skill_levels(&lvl), [0, 0, 5, 5]);
    }
  }

  mod seed_races {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, repo::sde};

    #[tokio::test]
    async fn it_seeds_races_and_defaults_a_missing_alliance_to_zero() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("races.yaml");
      tokio::fs::write(
        &path,
        "1: { name: { en: Caldari }, allianceID: 500001 }\n4: { name: { en: Jove } }\n",
      )
      .await
      .unwrap();
      let db = store::open_test().await.unwrap();

      seed_races(&db, &path).await.unwrap();

      let caldari = sde::get_race(&db, 1).await.unwrap().unwrap();
      assert_eq!(caldari.name(), "Caldari");
      assert_eq!(caldari.alliance_id(), 500_001);

      let jove = sde::get_race(&db, 4).await.unwrap().unwrap();
      assert_eq!(jove.alliance_id(), 0);
    }
  }

  mod seed_bloodlines {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, model::Race, repo::sde};

    async fn seed_parent_race(db: &Database) {
      sde::upsert_race(db, &Race::new(1, 0, "Caldari", "Caldari"))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_seeds_bloodlines_and_sets_a_present_ship_type_id() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("bloodlines.yaml");
      tokio::fs::write(
        &path,
        "5: { name: { en: Deteis }, raceID: 1, corporationID: 1000035, shipTypeID: 596, \
        charisma: 6, intelligence: 9, memory: 7, perception: 4, willpower: 4 }\n",
      )
      .await
      .unwrap();
      let db = store::open_test().await.unwrap();
      seed_parent_race(&db).await;

      seed_bloodlines(&db, &path).await.unwrap();

      let deteis = sde::get_bloodline(&db, 5).await.unwrap().unwrap();
      assert_eq!(deteis.name, "Deteis");
      assert_eq!(deteis.race_id, 1);
      assert_eq!(deteis.ship_type_id, Some(596));
      assert_eq!(deteis.charisma, 6);
    }

    #[tokio::test]
    async fn it_leaves_ship_type_id_null_when_the_sde_value_is_zero() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("bloodlines.yaml");
      tokio::fs::write(
        &path,
        "5: { name: { en: Deteis }, raceID: 1, corporationID: 1000035 }\n",
      )
      .await
      .unwrap();
      let db = store::open_test().await.unwrap();
      seed_parent_race(&db).await;

      seed_bloodlines(&db, &path).await.unwrap();

      let deteis = sde::get_bloodline(&db, 5).await.unwrap().unwrap();
      assert_eq!(deteis.ship_type_id, None);
    }
  }

  mod seed_factions {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, repo::sde};

    #[tokio::test]
    async fn it_seeds_factions_with_solar_system_and_unique_flag() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("factions.yaml");
      tokio::fs::write(
        &path,
        "500001: { name: { en: Caldari State }, sizeFactor: 5.5, solarSystemID: 30000145, isUnique: true }\n\
        500024: { name: { en: Generic } }\n",
      )
      .await
      .unwrap();
      let db = store::open_test().await.unwrap();

      seed_factions(&db, &path).await.unwrap();

      let state = sde::get_faction(&db, 500_001).await.unwrap().unwrap();
      assert_eq!(state.name, "Caldari State");
      assert_eq!(state.size_factor, 5.5);
      assert_eq!(state.solar_system_id, Some(30_000_145));
      assert_eq!(state.is_unique, 1);
      assert_eq!(state.station_count, 0);

      let generic = sde::get_faction(&db, 500_024).await.unwrap().unwrap();
      assert_eq!(generic.size_factor, 1.0);
      assert_eq!(generic.is_unique, 0);
      assert_eq!(generic.solar_system_id, None);
    }
  }

  mod seed_certificates {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, repo::skills};

    async fn seed_parent_skills(db: &Database, dir: &Path, ids: &[i64]) {
      tokio::fs::write(dir.join("categories.yaml"), "16: { name: { en: Skill } }\n")
        .await
        .unwrap();
      tokio::fs::write(
        dir.join("groups.yaml"),
        "255: { categoryID: 16, name: { en: Gunnery } }\n",
      )
      .await
      .unwrap();
      let types: String = ids
        .iter()
        .map(|id| format!("{id}: {{ groupID: 255, name: {{ en: Skill }} }}\n"))
        .collect();
      tokio::fs::write(dir.join("types.yaml"), types).await.unwrap();
      tokio::fs::write(dir.join("typeDogma.yaml"), "{}\n").await.unwrap();

      seed_categories(db, &dir.join("categories.yaml")).await.unwrap();
      seed_groups(db, &dir.join("groups.yaml")).await.unwrap();
      seed_types(
        db,
        &dir.join("types.yaml"),
        &dir.join("typeDogma.yaml"),
        &dir.join("groups.yaml"),
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_seeds_certificates_with_their_per_skill_levels() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("certificates.yaml");
      tokio::fs::write(
        &path,
        "1001:\n  name: { en: Core Fitting }\n  description: { en: Basic fitting }\n  grade: 3\n  \
        skillTypes:\n    3300: { basic: 1, improved: 3, advanced: 4, elite: 5 }\n    \
        3301: { basic: 2 }\n",
      )
      .await
      .unwrap();
      let db = store::open_test().await.unwrap();
      seed_parent_skills(&db, tmp.path(), &[3300, 3301]).await;

      seed_certificates(&db, &path).await.unwrap();

      let cert = skills::by_ids(&db, &[1001]).await.unwrap();
      assert_eq!(cert.len(), 1);
      assert_eq!(cert[0].grade(), 3);
      assert_eq!(cert[0].name(), "Core Fitting");

      let mut skills = skills::skills_for(&db, 1001).await.unwrap();
      skills.sort_by_key(|s| s.skill_id());
      assert_eq!(skills.len(), 2);
      assert_eq!(skills[0].skill_id(), 3300);
      assert_eq!(skills[0].basic(), 1);
      assert_eq!(skills[0].improved(), 3);
      assert_eq!(skills[0].advanced(), 4);
      assert_eq!(skills[0].elite(), 5);
    }

    #[tokio::test]
    async fn it_defaults_and_clamps_grade_into_one_to_five() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("certificates.yaml");
      tokio::fs::write(
        &path,
        "1001:\n  name: { en: NoGrade }\n1002:\n  name: { en: TooHigh }\n  grade: 9\n",
      )
      .await
      .unwrap();
      let db = store::open_test().await.unwrap();

      seed_certificates(&db, &path).await.unwrap();

      let certs = skills::by_ids(&db, &[1001, 1002]).await.unwrap();
      let by_id = |id: i64| certs.iter().find(|c| c.id() == id).unwrap();
      assert_eq!(by_id(1001).grade(), 1);
      assert_eq!(by_id(1002).grade(), 5);
    }
  }

  mod seed_masteries {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, model::Certificate, repo::skills};

    async fn seed_parents(db: &Database, dir: &Path) {
      tokio::fs::write(dir.join("categories.yaml"), "25: { name: { en: Ship } }\n")
        .await
        .unwrap();
      tokio::fs::write(
        dir.join("groups.yaml"),
        "25: { categoryID: 25, name: { en: Frigate } }\n",
      )
      .await
      .unwrap();
      tokio::fs::write(dir.join("types.yaml"), "596: { groupID: 25, name: { en: Impairor } }\n")
        .await
        .unwrap();
      tokio::fs::write(dir.join("typeDogma.yaml"), "{}\n").await.unwrap();
      seed_categories(db, &dir.join("categories.yaml")).await.unwrap();
      seed_groups(db, &dir.join("groups.yaml")).await.unwrap();
      seed_types(
        db,
        &dir.join("types.yaml"),
        &dir.join("typeDogma.yaml"),
        &dir.join("groups.yaml"),
      )
      .await
      .unwrap();
      skills::certificate_upsert_many(
        db,
        &[Certificate {
          description: None,
          grade: 1,
          id: 100,
          name: "Cert".to_owned(),
        }],
        &[],
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_seeds_one_mastery_row_per_cert_at_each_tier() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("masteries.yaml");
      tokio::fs::write(&path, "596:\n  1:\n    - 100\n").await.unwrap();
      let db = store::open_test().await.unwrap();
      seed_parents(&db, tmp.path()).await;

      seed_masteries(&db, &path).await.unwrap();

      let masteries = skills::for_ship(&db, 596).await.unwrap();
      assert_eq!(masteries.len(), 1);
      assert_eq!(masteries[0].certificate_id(), 100);
      assert_eq!(masteries[0].tier(), 2);
    }

    #[tokio::test]
    async fn it_is_a_noop_when_the_file_has_no_usable_rows() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("masteries.yaml");
      tokio::fs::write(&path, "- just-a-list\n").await.unwrap();
      let db = store::open_test().await.unwrap();

      seed_masteries(&db, &path).await.unwrap();

      let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ship_masteries")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(count, 0);
    }
  }

  mod seed_all_tables {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      repo::{sde, skills},
    };

    async fn write_yaml(dir: &Path, name: &str, body: &str) {
      tokio::fs::write(dir.join(name), body).await.unwrap();
    }

    async fn write_full_fixture(dir: &Path) {
      write_yaml(
        dir,
        "categories.yaml",
        "16: { name: { en: Skill }, published: true }\n25: { name: { en: Ship }, published: true }\n",
      )
      .await;
      write_yaml(
        dir,
        "groups.yaml",
        "255: { categoryID: 16, name: { en: Gunnery }, published: true }\n\
        25: { categoryID: 25, name: { en: Frigate }, published: true }\n",
      )
      .await;
      write_yaml(
        dir,
        "marketGroups.yaml",
        "9: { name: { en: Ships }, hasTypes: false }\n",
      )
      .await;
      write_yaml(
        dir,
        "types.yaml",
        "3300: { groupID: 255, name: { en: Gunnery }, published: true }\n\
        596: { groupID: 25, name: { en: Impairor }, published: true }\n",
      )
      .await;
      write_yaml(
        dir,
        "typeDogma.yaml",
        "3300:\n  dogmaAttributes:\n    - { attributeID: 275, value: 1.0 }\n    \
        - { attributeID: 180, value: 167.0 }\n    - { attributeID: 181, value: 166.0 }\n",
      )
      .await;
      write_yaml(
        dir,
        "dogmaAttributes.yaml",
        "4:\n  name: mass\n  displayName: { en: Mass }\n  defaultValue: 0.0\n  highIsGood: false\n  \
        unitID: 2\n  iconID: 100\n  published: true\n  stackable: true\n",
      )
      .await;
      write_yaml(dir, "races.yaml", "1: { name: { en: Caldari }, allianceID: 500001 }\n").await;
      write_yaml(
        dir,
        "bloodlines.yaml",
        "5: { name: { en: Deteis }, raceID: 1, corporationID: 1000035, shipTypeID: 596 }\n",
      )
      .await;
      write_yaml(
        dir,
        "factions.yaml",
        "500001: { name: { en: Caldari State }, solarSystemID: 30000145, isUnique: true }\n",
      )
      .await;
      write_yaml(
        dir,
        "certificates.yaml",
        "100:\n  name: { en: Core Fitting }\n  grade: 1\n  skillTypes:\n    3300: { basic: 1 }\n",
      )
      .await;
      write_yaml(dir, "masteries.yaml", "596:\n  1:\n    - 100\n").await;
    }

    fn channel() -> (Tx, iced::futures::channel::mpsc::Receiver<Progress>) {
      iced::futures::channel::mpsc::channel(64)
    }

    #[tokio::test]
    async fn it_seeds_every_in_scope_table_in_fk_order() {
      let tmp = tempfile::tempdir().unwrap();
      write_full_fixture(tmp.path()).await;
      let db = store::open_test().await.unwrap();
      let (mut tx, _rx) = channel();

      seed_all_tables(&db, &mut tx, tmp.path()).await.unwrap();

      assert!(sde::get_race(&db, 1).await.unwrap().is_some());
      assert!(sde::get_bloodline(&db, 5).await.unwrap().is_some());
      assert!(sde::get_faction(&db, 500_001).await.unwrap().is_some());
      assert_eq!(skills::by_ids(&db, &[100]).await.unwrap().len(), 1);
      assert_eq!(skills::for_ship(&db, 596).await.unwrap().len(), 1);

      let mass = sde::get_dogma_attribute(&db, 4).await.unwrap().unwrap();
      assert_eq!(mass.name(), "mass");
      assert_eq!(mass.display_name().as_deref(), Some("Mass"));
      assert_eq!(mass.high_is_good(), false);
      assert_eq!(mass.unit_id(), Some(2));
    }

    #[tokio::test]
    async fn it_silently_skips_the_optional_files_when_absent() {
      let tmp = tempfile::tempdir().unwrap();
      write_full_fixture(tmp.path()).await;
      tokio::fs::remove_file(tmp.path().join("certificates.yaml"))
        .await
        .unwrap();
      tokio::fs::remove_file(tmp.path().join("masteries.yaml")).await.unwrap();
      let db = store::open_test().await.unwrap();
      let (mut tx, _rx) = channel();

      seed_all_tables(&db, &mut tx, tmp.path()).await.unwrap();

      assert!(skills::by_ids(&db, &[100]).await.unwrap().is_empty());
      assert!(skills::for_ship(&db, 596).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn seed_if_stale_writes_the_version_marker_when_stale() {
      let tmp = tempfile::tempdir().unwrap();
      write_full_fixture(tmp.path()).await;
      let marker = tmp.path().join("sde_version");
      let db = store::open_test().await.unwrap();
      let (mut tx, _rx) = channel();

      seed_if_stale_at(&db, &mut tx, tmp.path(), Some("20240101.1"), Some(&marker))
        .await
        .unwrap();

      assert_eq!(read_stored_sde_version(&marker), Some(composite_version("20240101.1")));
      assert!(sde::get_race(&db, 1).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn seed_if_stale_skips_seeding_when_the_stored_version_matches() {
      let tmp = tempfile::tempdir().unwrap();
      write_full_fixture(tmp.path()).await;
      let marker = tmp.path().join("sde_version");
      write_stored_sde_version(&marker, &composite_version("20240101.1"));
      let db = store::open_test().await.unwrap();
      let (mut tx, _rx) = channel();

      seed_if_stale_at(&db, &mut tx, tmp.path(), Some("20240101.1"), Some(&marker))
        .await
        .unwrap();

      assert!(sde::get_race(&db, 1).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn seed_if_stale_backfills_dogma_attributes_when_current_but_unseeded() {
      let tmp = tempfile::tempdir().unwrap();
      write_full_fixture(tmp.path()).await;
      let marker = tmp.path().join("sde_version");
      write_stored_sde_version(&marker, &composite_version("20240101.1"));
      let db = store::open_test().await.unwrap();
      let (mut tx, _rx) = channel();

      seed_if_stale_at(&db, &mut tx, tmp.path(), Some("20240101.1"), Some(&marker))
        .await
        .unwrap();

      assert!(sde::get_dogma_attribute(&db, 4).await.unwrap().is_some());
      assert!(sde::get_race(&db, 1).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn seed_if_stale_seeds_but_writes_no_marker_for_a_versionless_build() {
      let tmp = tempfile::tempdir().unwrap();
      write_full_fixture(tmp.path()).await;
      let marker = tmp.path().join("sde_version");
      let db = store::open_test().await.unwrap();
      let (mut tx, _rx) = channel();

      seed_if_stale_at(&db, &mut tx, tmp.path(), None, Some(&marker))
        .await
        .unwrap();

      assert!(sde::get_race(&db, 1).await.unwrap().is_some());
      assert!(!marker.exists());
    }
  }

  mod sde_is_current {
    use super::*;

    #[test]
    fn it_reports_current_when_the_marker_matches_the_composite() {
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("sde_version");
      let composite = composite_version("20240101.1");
      write_stored_sde_version(&marker, &composite);

      assert!(sde_is_current(Some(&marker), Some(&composite)));
    }

    #[test]
    fn it_reports_stale_when_the_marker_differs() {
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("sde_version");
      write_stored_sde_version(&marker, &composite_version("20240101.1"));

      assert!(!sde_is_current(Some(&marker), Some(&composite_version("20240102.1"))));
    }

    #[test]
    fn it_reports_stale_when_the_marker_is_absent() {
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("sde_version");

      assert!(!sde_is_current(Some(&marker), Some("20240101.1+pod-0.5.0+seed-2")));
    }

    #[test]
    fn it_reports_stale_for_a_versionless_build() {
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("sde_version");
      write_stored_sde_version(&marker, &composite_version("20240101.1"));

      assert!(!sde_is_current(Some(&marker), None));
    }
  }

  mod seed_abyssal_module_stats {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, repo::assets};

    const FIXTURE: &str = "47405:\n  \
      attributeIDs:\n    6: { min: 0.6, max: 1.4 }\n    30: { min: 0.9, max: 1.1 }\n  \
      inputOutputMapping:\n    - { applicableTypes: [12058], resultingType: 47408 }\n    \
      - { applicableTypes: [12060], resultingType: 47410 }\n";

    #[tokio::test]
    async fn it_seeds_one_bound_row_per_resulting_type_and_attribute() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("dynamicItemAttributes.yaml");
      tokio::fs::write(&path, FIXTURE).await.unwrap();
      let db = store::open_test().await.unwrap();

      seed_abyssal_module_stats(&db, &path).await.unwrap();

      let stats = assets::module_stats_for_type(&db, 47408).await.unwrap();
      assert_eq!(stats.len(), 2);
      let attr6 = stats.iter().find(|s| s.attribute_id() == 6).unwrap();
      assert_eq!(attr6.min_mult(), 0.6);
      assert_eq!(attr6.max_mult(), 1.4);

      let mut ids = assets::abyssal_type_ids(&db).await.unwrap();
      ids.sort_unstable();
      assert_eq!(ids, [47408, 47410]);
    }

    #[tokio::test]
    async fn it_is_idempotent_across_reseed() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("dynamicItemAttributes.yaml");
      tokio::fs::write(&path, FIXTURE).await.unwrap();
      let db = store::open_test().await.unwrap();

      seed_abyssal_module_stats(&db, &path).await.unwrap();
      seed_abyssal_module_stats(&db, &path).await.unwrap();

      let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM abyssal_module_stats")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(count, 4);
    }
  }
}
