use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

use iced::{Task, futures::SinkExt as _};
use serde::Deserialize;

use crate::{
  clients::{esi::models::universe::DogmaAttribute, http, sde as sde_client},
  config,
  services::i18n::Language,
  store::{
    Database,
    model::{
      AbyssalModuleStat, AgentType, Bloodline, Certificate, CertificateSkill, Constellation,
      DogmaAttribute as DogmaAttributeMeta, Faction, ItemCategory, ItemGroup, ItemType, MarketGroup, Moon, NpcAgent,
      NpcAgentSkill, NpcCorporationDivision, Race, Region, SeedCorporation, ShipMastery, SkillMetadata, SolarSystem,
      Station, TypeMaterial,
    },
    repo::{assets, org, sde, skills},
  },
};

const SEED_FORMAT_REVISION: u32 = 9;

const SKILL_CATEGORY_ID: i64 = 16;

const SKILL_RANK_ATTR_ID: i32 = 275;

const SKILL_PRIMARY_ATTR_ID: i32 = 180;

const SKILL_SECONDARY_ATTR_ID: i32 = 181;

/// Number of bind parameters SQLite allows in a single prepared statement. Both blueprint tables bind
/// four columns per row, so a chunk caps at `SQLITE_MAX_BIND_PARAMS / 4` rows.
const SQLITE_MAX_BIND_PARAMS: usize = 999;

#[derive(Clone, Debug)]
pub enum Progress {
  Complete,
  Degraded(String),
  Error(String),
  Step(String),
}

/// One `(blueprint_type_id, activity_id, time, max_production_limit)` row for `blueprint_activity_meta`.
/// `time` is base seconds per run; `max_production_limit` is the per-job run cap (0 when the SDE omits it).
#[derive(Clone, Debug, Eq, PartialEq)]
struct BlueprintActivityMetaRow {
  activity_id: i64,
  blueprint_type_id: i64,
  max_production_limit: i64,
  time: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BlueprintActivityRow {
  activity_id: i64,
  blueprint_type_id: i64,
  quantity: i64,
  type_id: i64,
}

#[derive(Clone, Default, Deserialize)]
struct LocalizedString {
  de: Option<String>,
  en: Option<String>,
  es: Option<String>,
  fr: Option<String>,
  ja: Option<String>,
  ko: Option<String>,
  ru: Option<String>,
  zh: Option<String>,
}

impl LocalizedString {
  fn field(&self, sde_code: &str) -> Option<&str> {
    let field = match sde_code {
      "de" => &self.de,
      "en" => &self.en,
      "es" => &self.es,
      "fr" => &self.fr,
      "ja" => &self.ja,
      "ko" => &self.ko,
      "ru" => &self.ru,
      "zh" => &self.zh,
      _ => &self.en,
    };
    field.as_deref().filter(|value| !value.is_empty())
  }

  fn pick(&self, language: Language) -> String {
    self
      .field(language.sde_code())
      .or_else(|| self.field("en"))
      .unwrap_or_default()
      .to_owned()
  }
}

#[derive(Deserialize)]
struct SdeAgentEntry {
  #[serde(rename = "agentTypeID")]
  agent_type_id: Option<i64>,
  #[serde(rename = "divisionID")]
  division_id: Option<i64>,
  #[serde(rename = "isLocator", default)]
  is_locator: bool,
  level: Option<i64>,
}

#[derive(Deserialize)]
struct SdeAgentSkillEntry {
  #[serde(rename = "typeID")]
  type_id: i64,
}

#[derive(Deserialize)]
struct SdeAgentTypeEntry {
  #[serde(rename = "_key")]
  id: i64,
  name: String,
}

#[derive(Deserialize)]
struct SdeBloodlineEntry {
  #[serde(default)]
  charisma: i32,
  #[serde(rename = "corporationID", default)]
  corporation_id: i32,
  #[serde(rename = "_key")]
  id: i64,
  #[serde(default)]
  intelligence: i32,
  #[serde(default)]
  memory: i32,
  name: Option<LocalizedString>,
  #[serde(default)]
  perception: i32,
  #[serde(rename = "raceID", default)]
  race_id: i32,
  #[serde(default)]
  willpower: i32,
}

#[derive(Deserialize)]
struct SdeBlueprintActivity {
  #[serde(default)]
  materials: Vec<SdeBlueprintQuantity>,
  #[serde(default)]
  products: Vec<SdeBlueprintQuantity>,
  #[serde(default)]
  time: Option<i64>,
}

#[derive(Deserialize)]
struct SdeBlueprintEntry {
  #[serde(default)]
  activities: HashMap<String, SdeBlueprintActivity>,
  #[serde(rename = "_key")]
  id: i64,
  #[serde(rename = "maxProductionLimit", default)]
  max_production_limit: i64,
}

#[derive(Deserialize)]
struct SdeBlueprintQuantity {
  quantity: i64,
  #[serde(rename = "typeID")]
  type_id: i64,
}

#[derive(Deserialize)]
struct SdeCategoryEntry {
  #[serde(rename = "iconID")]
  icon_id: Option<i64>,
  #[serde(rename = "_key")]
  id: i64,
  name: LocalizedString,
  #[serde(default = "default_true")]
  published: bool,
}

#[derive(Deserialize)]
struct SdeCertEntry {
  #[serde(default)]
  description: Option<LocalizedString>,
  #[serde(rename = "_key")]
  id: i64,
  name: LocalizedString,
  #[serde(rename = "skillTypes", default)]
  skill_types: Vec<SdeCertSkill>,
}

#[derive(Deserialize)]
struct SdeCertSkill {
  #[serde(default)]
  advanced: i32,
  #[serde(default)]
  basic: i32,
  #[serde(default)]
  elite: i32,
  #[serde(default)]
  improved: i32,
  #[serde(rename = "_key")]
  skill_id: i32,
}

#[derive(Deserialize)]
struct SdeConstellationEntry {
  #[serde(rename = "_key")]
  id: i64,
  name: Option<LocalizedString>,
  position: SdePosition,
  #[serde(rename = "regionID")]
  region_id: i64,
}

#[derive(Deserialize)]
struct SdeDogmaAttrEntry {
  #[serde(rename = "defaultValue")]
  default_value: Option<f64>,
  #[serde(default)]
  description: Option<String>,
  #[serde(rename = "displayName")]
  display_name: Option<LocalizedString>,
  #[serde(rename = "highIsGood", default)]
  high_is_good: bool,
  #[serde(rename = "iconID")]
  icon_id: Option<i64>,
  #[serde(rename = "_key")]
  id: i64,
  name: String,
  #[serde(default)]
  published: bool,
  #[serde(default = "default_true")]
  stackable: bool,
  #[serde(rename = "unitID")]
  unit_id: Option<i64>,
}

#[derive(Deserialize)]
struct SdeDogmaAttribute {
  #[serde(rename = "attributeID")]
  attribute_id: i32,
  value: f64,
}

#[derive(Deserialize)]
struct SdeDynamicAttrBounds {
  #[serde(rename = "_key")]
  attribute_id: i32,
  max: f64,
  min: f64,
}

#[derive(Deserialize)]
struct SdeDynamicEntry {
  #[serde(rename = "attributeIDs", default)]
  attribute_ids: Vec<SdeDynamicAttrBounds>,
  #[serde(rename = "inputOutputMapping", default)]
  input_output_mapping: Vec<SdeDynamicMapping>,
}

#[derive(Deserialize)]
struct SdeDynamicMapping {
  #[serde(rename = "resultingType")]
  resulting_type: i32,
}

#[derive(Deserialize)]
struct SdeFactionEntry {
  #[serde(rename = "_key")]
  id: i64,
  name: Option<LocalizedString>,
  #[serde(rename = "sizeFactor", default = "default_one_f64")]
  size_factor: f64,
  #[serde(rename = "solarSystemID")]
  solar_system_id: Option<i32>,
}

#[derive(Deserialize)]
struct SdeGroupEntry {
  #[serde(rename = "categoryID")]
  category_id: i64,
  #[serde(rename = "iconID")]
  icon_id: Option<i64>,
  #[serde(rename = "_key")]
  id: i64,
  name: LocalizedString,
  #[serde(default = "default_true")]
  published: bool,
}

#[derive(Deserialize)]
struct SdeMapMoonEntry {
  #[serde(rename = "_key")]
  id: i64,
  #[serde(rename = "orbitID")]
  orbit_id: i64,
  #[serde(rename = "orbitIndex")]
  orbit_index: i32,
  position: Option<SdePosition>,
  radius: Option<f64>,
  #[serde(rename = "solarSystemID")]
  solar_system_id: Option<i64>,
  #[serde(rename = "typeID")]
  type_id: Option<i64>,
}

#[derive(Deserialize)]
struct SdeMapPlanetEntry {
  #[serde(rename = "celestialIndex")]
  celestial_index: i32,
  #[serde(rename = "_key")]
  id: i64,
  #[serde(rename = "solarSystemID")]
  solar_system_id: i64,
}

#[derive(Deserialize)]
struct SdeMarketGroupEntry {
  description: Option<LocalizedString>,
  #[serde(rename = "hasTypes")]
  has_types: Option<bool>,
  #[serde(rename = "iconID")]
  icon_id: Option<i64>,
  #[serde(rename = "_key")]
  id: i64,
  name: Option<LocalizedString>,
  #[serde(rename = "parentGroupID")]
  parent_group_id: Option<i64>,
}

#[derive(Deserialize)]
struct SdeMasteryEntry {
  #[serde(rename = "_key")]
  ship_type_id: i64,
  #[serde(rename = "_value", default)]
  tiers: Vec<SdeMasteryTier>,
}

#[derive(Deserialize)]
struct SdeMasteryTier {
  #[serde(rename = "_value", default)]
  certificate_ids: Vec<i64>,
  #[serde(rename = "_key")]
  tier: i64,
}

#[derive(Deserialize)]
struct SdeNpcCharacterEntry {
  agent: Option<SdeAgentEntry>,
  #[serde(rename = "corporationID")]
  corporation_id: Option<i64>,
  #[serde(rename = "_key")]
  id: i64,
  #[serde(rename = "locationID")]
  location_id: Option<i64>,
  name: Option<LocalizedString>,
  #[serde(default)]
  skills: Vec<SdeAgentSkillEntry>,
}

#[derive(Deserialize)]
struct SdeNpcCorporationDivisionEntry {
  #[serde(rename = "_key")]
  id: i64,
  name: Option<LocalizedString>,
}

#[derive(Deserialize)]
struct SdeNpcCorporationEntry {
  #[serde(rename = "factionID")]
  faction_id: Option<i64>,
  #[serde(rename = "_key")]
  id: i64,
  name: Option<LocalizedString>,
  #[serde(rename = "stationID")]
  station_id: Option<i64>,
  #[serde(rename = "tickerName")]
  ticker_name: Option<String>,
}

#[derive(Deserialize)]
struct SdeNpcStationEntry {
  #[serde(rename = "_key")]
  id: i64,
  #[serde(rename = "operationID")]
  operation_id: Option<i64>,
  #[serde(rename = "orbitID")]
  orbit_id: Option<i64>,
  #[serde(rename = "ownerID")]
  owner_id: Option<i64>,
  position: SdePosition,
  #[serde(rename = "reprocessingEfficiency", default)]
  reprocessing_efficiency: f64,
  #[serde(rename = "reprocessingStationsTake", default)]
  reprocessing_stations_take: f64,
  #[serde(rename = "solarSystemID")]
  solar_system_id: i64,
  #[serde(rename = "typeID")]
  type_id: i64,
  #[serde(rename = "useOperationName", default)]
  use_operation_name: bool,
}

#[derive(Deserialize)]
struct SdePlanetSchematicEntry {
  #[serde(rename = "cycleTime", default)]
  cycle_time: i64,
  #[serde(rename = "_key")]
  id: i64,
  name: Option<LocalizedString>,
  #[serde(default)]
  types: Vec<SdePlanetSchematicType>,
}

#[derive(Deserialize)]
struct SdePlanetSchematicType {
  #[serde(rename = "isInput", default)]
  is_input: bool,
  #[serde(default)]
  quantity: i64,
  #[serde(rename = "_key")]
  type_id: i64,
}

#[derive(Deserialize)]
struct SdePosition {
  x: f64,
  y: f64,
  z: f64,
}

#[derive(Deserialize)]
struct SdeRaceEntry {
  #[serde(rename = "_key")]
  id: i64,
  name: Option<LocalizedString>,
}

#[derive(Deserialize)]
struct SdeRegionEntry {
  #[serde(rename = "_key")]
  id: i64,
  name: Option<LocalizedString>,
}

#[derive(Deserialize)]
struct SdeSolarSystemEntry {
  #[serde(rename = "constellationID")]
  constellation_id: i64,
  #[serde(rename = "_key")]
  id: i64,
  name: Option<LocalizedString>,
  position: SdePosition,
  #[serde(rename = "securityClass")]
  security_class: Option<String>,
  #[serde(rename = "securityStatus")]
  security_status: f64,
  #[serde(rename = "starID")]
  star_id: Option<i64>,
}

#[derive(Deserialize)]
struct SdeStationOperationEntry {
  #[serde(rename = "_key")]
  id: i64,
  #[serde(rename = "operationName")]
  operation_name: Option<LocalizedString>,
}

#[derive(Deserialize)]
struct SdeTypeDogmaEntry {
  #[serde(rename = "dogmaAttributes", default)]
  dogma_attributes: Vec<SdeDogmaAttribute>,
  #[serde(rename = "_key")]
  id: i64,
}

#[derive(Deserialize)]
struct SdeTypeEntry {
  capacity: Option<f64>,
  description: Option<LocalizedString>,
  #[serde(rename = "groupID")]
  group_id: i64,
  #[serde(rename = "iconID")]
  icon_id: Option<i64>,
  #[serde(rename = "_key")]
  id: i64,
  #[serde(rename = "marketGroupID")]
  market_group_id: Option<i64>,
  name: LocalizedString,
  #[serde(rename = "portionSize")]
  portion_size: Option<i32>,
  #[serde(default = "default_true")]
  published: bool,
  radius: Option<f64>,
  volume: Option<f64>,
}

#[derive(Deserialize)]
struct SdeTypeMaterialEntry {
  #[serde(rename = "_key")]
  id: i64,
  #[serde(default)]
  materials: Vec<SdeTypeMaterialQuantity>,
}

#[derive(Deserialize)]
struct SdeTypeMaterialQuantity {
  #[serde(rename = "materialTypeID")]
  material_type_id: i64,
  quantity: i64,
}

type Tx = iced::futures::channel::mpsc::Sender<Progress>;

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
      let message = format!("SDE seed error: {e}");
      let progress = if sde::is_seeded(&db).await.unwrap_or(false) {
        Progress::Degraded(message)
      } else {
        Progress::Error(message)
      };
      let _ = tx.send(progress).await;
    }
  }
}

async fn do_seed(db: &Database, http: Arc<http::Client>, tx: &mut Tx) -> Result<(), String> {
  let client = sde_client::Client::new(http);
  let language = configured_language();

  let latest_build = client.latest_build_version().await;
  let seeded = sde::is_seeded(db).await.unwrap_or(false);
  if should_skip_download(latest_build.as_deref(), sde_version_path().as_deref(), seeded, language) {
    tracing::info!(target: "pod::sde", build = latest_build.as_deref(), "SDE already current; skipping full download");
    return Ok(());
  }

  step(tx, &t!("splash.seed.downloading_static_data")).await;
  let extracted = client.download_and_extract().await.map_err(|e| e.to_string())?;

  seed_if_stale(db, tx, &extracted.root, extracted.build_version.as_deref(), language).await
}

fn configured_language() -> Language {
  config::load()
    .map(|settings| settings.accessibility().language())
    .unwrap_or_default()
}

fn should_skip_download(
  latest_build: Option<&str>,
  marker_path: Option<&Path>,
  seeded: bool,
  language: Language,
) -> bool {
  let Some(build) = latest_build else {
    return false;
  };

  seeded && sde_is_current(marker_path, Some(&composite_version(build, language)))
}

async fn seed_if_stale(
  db: &Database,
  tx: &mut Tx,
  root: &Path,
  build_version: Option<&str>,
  language: Language,
) -> Result<(), String> {
  seed_if_stale_at(db, tx, root, build_version, sde_version_path().as_deref(), language).await
}

async fn seed_if_stale_at(
  db: &Database,
  tx: &mut Tx,
  root: &Path,
  build_version: Option<&str>,
  marker_path: Option<&Path>,
  language: Language,
) -> Result<(), String> {
  let composite = build_version.map(|build| composite_version(build, language));
  if sde_is_current(marker_path, composite.as_deref()) {
    backfill_dogma_attributes(db, tx, root, language).await?;
    return Ok(());
  }

  seed_all_tables(db, tx, root, language).await?;

  if let (Some(path), Some(version)) = (marker_path, composite.as_deref()) {
    write_stored_sde_version(path, version);
  }

  Ok(())
}

async fn backfill_dogma_attributes(db: &Database, tx: &mut Tx, root: &Path, language: Language) -> Result<(), String> {
  if sde::is_seeded(db).await.map_err(|e| e.to_string())? {
    return Ok(());
  }

  let path = root.join("dogmaAttributes.jsonl");
  if path.exists() {
    step(tx, &t!("splash.seed.backfilling_dogma_attributes")).await;
    seed_dogma_attributes(db, &path, language).await?;
  }

  Ok(())
}

fn sde_is_current(marker_path: Option<&Path>, composite: Option<&str>) -> bool {
  let (Some(marker_path), Some(composite)) = (marker_path, composite) else {
    return false;
  };
  read_stored_sde_version(marker_path).as_deref() == Some(composite)
}

async fn seed_all_tables(db: &Database, tx: &mut Tx, r: &Path, language: Language) -> Result<(), String> {
  step(tx, &t!("splash.seed.item_categories")).await;
  seed_categories(db, &r.join("categories.jsonl"), language).await?;

  step(tx, &t!("splash.seed.item_groups")).await;
  seed_groups(db, &r.join("groups.jsonl"), language).await?;

  step(tx, &t!("splash.seed.market_groups")).await;
  seed_market_groups(db, &r.join("marketGroups.jsonl"), language).await?;

  step(tx, &t!("splash.seed.item_types")).await;
  seed_types(
    db,
    &r.join("types.jsonl"),
    &r.join("typeDogma.jsonl"),
    &r.join("groups.jsonl"),
    language,
  )
  .await?;

  step(tx, &t!("splash.seed.dogma_attributes")).await;
  seed_dogma_attributes(db, &r.join("dogmaAttributes.jsonl"), language).await?;

  let dynamic_path = r.join("dynamicItemAttributes.jsonl");
  if dynamic_path.exists() {
    step(tx, &t!("splash.seed.abyssal_module_stats")).await;
    seed_abyssal_module_stats(db, &dynamic_path).await?;
  }

  step(tx, &t!("splash.seed.races")).await;
  seed_races(db, &r.join("races.jsonl"), language).await?;

  step(tx, &t!("splash.seed.bloodlines")).await;
  seed_bloodlines(db, &r.join("bloodlines.jsonl"), language).await?;

  step(tx, &t!("splash.seed.factions")).await;
  seed_factions(db, &r.join("factions.jsonl"), language).await?;

  let cert_path = r.join("certificates.jsonl");
  if cert_path.exists() {
    step(tx, &t!("splash.seed.certificates")).await;
    seed_certificates(db, &cert_path, language).await?;
  }

  let mastery_path = r.join("masteries.jsonl");
  if mastery_path.exists() {
    step(tx, &t!("splash.seed.ship_masteries")).await;
    seed_masteries(db, &mastery_path).await?;
  }

  step(tx, &t!("splash.seed.npc_corporations")).await;
  seed_npc_corporations(db, &r.join("npcCorporations.jsonl"), language).await?;

  step(tx, &t!("splash.seed.regions")).await;
  seed_regions(db, &r.join("mapRegions.jsonl"), language).await?;

  step(tx, &t!("splash.seed.constellations")).await;
  seed_constellations(db, &r.join("mapConstellations.jsonl"), language).await?;

  step(tx, &t!("splash.seed.solar_systems")).await;
  seed_solar_systems(db, &r.join("mapSolarSystems.jsonl"), language).await?;

  step(tx, &t!("splash.seed.npc_stations")).await;
  seed_npc_stations(db, r, language).await?;

  step(tx, &t!("splash.seed.moons")).await;
  seed_moons(db, r).await?;

  step(tx, &t!("splash.seed.agent_types")).await;
  seed_agent_types(db, &r.join("agentTypes.jsonl")).await?;

  step(tx, &t!("splash.seed.npc_corporation_divisions")).await;
  seed_npc_corporation_divisions(db, &r.join("npcCorporationDivisions.jsonl"), language).await?;

  step(tx, &t!("splash.seed.npc_agents")).await;
  seed_npc_agents(db, &r.join("npcCharacters.jsonl"), language).await?;

  seed_industry_static_tables(db, tx, r, language).await?;

  Ok(())
}

async fn seed_industry_static_tables(db: &Database, tx: &mut Tx, r: &Path, language: Language) -> Result<(), String> {
  let blueprints_path = r.join("blueprints.jsonl");
  if blueprints_path.exists() {
    step(tx, &t!("splash.seed.blueprints")).await;
    seed_blueprints(db, &blueprints_path).await?;
  }

  let type_materials_path = r.join("typeMaterials.jsonl");
  if type_materials_path.exists() {
    step(tx, &t!("splash.seed.type_materials")).await;
    seed_type_materials(db, &type_materials_path).await?;
  }

  let planet_schematics_path = r.join("planetSchematics.jsonl");
  if planet_schematics_path.exists() {
    step(tx, &t!("splash.seed.planet_schematics")).await;
    seed_planet_schematics(db, &planet_schematics_path, language).await?;
  }

  Ok(())
}

async fn step(tx: &mut Tx, label: &str) {
  let _ = tx.send(Progress::Step(label.to_string())).await;
}

async fn read_jsonl<T: serde::de::DeserializeOwned + Send + 'static>(path: &Path) -> Result<Vec<T>, String> {
  let path = path.to_owned();
  tokio::task::spawn_blocking(move || {
    let data = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    data
      .lines()
      .filter(|line| !line.trim().is_empty())
      .map(|line| serde_json::from_str::<T>(line).map_err(|e| format!("parse {}: {e}", path.display())))
      .collect()
  })
  .await
  .map_err(|e| e.to_string())?
}

async fn seed_categories(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdeCategoryEntry> = read_jsonl(path).await?;

  let records: Vec<ItemCategory> = entries
    .into_iter()
    .map(|e| ItemCategory {
      icon_id: e.icon_id,
      id: e.id,
      name: e.name.pick(language),
      published: e.published,
    })
    .collect();

  sde::upsert_many_item_categories(db, &records)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_groups(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdeGroupEntry> = read_jsonl(path).await?;

  let records: Vec<ItemGroup> = entries
    .into_iter()
    .map(|e| ItemGroup {
      category_id: e.category_id,
      icon_id: e.icon_id,
      id: e.id,
      name: e.name.pick(language),
      published: e.published,
    })
    .collect();

  sde::upsert_many_item_groups(db, &records)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_market_groups(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdeMarketGroupEntry> = read_jsonl(path).await?;

  let records: Vec<MarketGroup> = entries
    .into_iter()
    .map(|e| MarketGroup {
      description: e.description.map(|d| d.pick(language)).unwrap_or_default(),
      has_types: e.has_types.unwrap_or(false),
      icon_id: e.icon_id,
      id: e.id,
      name: e.name.map(|n| n.pick(language)).unwrap_or_default(),
      parent_id: e.parent_group_id,
    })
    .collect();

  sde::upsert_many_market_groups(db, &records)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_types(
  db: &Database,
  types_path: &Path,
  dogma_path: &Path,
  groups_path: &Path,
  language: Language,
) -> Result<(), String> {
  let entries: Vec<SdeTypeEntry> = read_jsonl(types_path).await?;
  let dogma: HashMap<i64, SdeTypeDogmaEntry> = read_jsonl::<SdeTypeDogmaEntry>(dogma_path)
    .await?
    .into_iter()
    .map(|d| (d.id, d))
    .collect();
  let groups: HashMap<i64, SdeGroupEntry> = read_jsonl::<SdeGroupEntry>(groups_path)
    .await?
    .into_iter()
    .map(|g| (g.id, g))
    .collect();

  let skill_metadata: Vec<SkillMetadata> = entries
    .iter()
    .filter(|e| {
      e.published
        && groups
          .get(&e.group_id)
          .is_some_and(|g| g.category_id == SKILL_CATEGORY_ID)
    })
    .filter_map(|e| build_skill_metadata(e.id, dogma.get(&e.id)))
    .collect();

  let records: Vec<ItemType> = entries
    .into_iter()
    .map(|e| {
      let d = dogma.get(&e.id);
      build_item_type(e, d, language)
    })
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

fn build_item_type(e: SdeTypeEntry, d: Option<&SdeTypeDogmaEntry>, language: Language) -> ItemType {
  ItemType {
    capacity: e.capacity,
    description: Some(e.description.map(|desc| desc.pick(language)).unwrap_or_default()),
    dogma_attributes: build_dogma_attributes_json(d),
    group_id: e.group_id,
    icon_id: e.icon_id,
    id: e.id,
    market_group_id: e.market_group_id,
    name: e.name.pick(language),
    packaged_volume: None,
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

fn build_dogma_attribute(e: SdeDogmaAttrEntry, language: Language) -> DogmaAttributeMeta {
  DogmaAttributeMeta {
    attribute_id: e.id,
    default_value: e.default_value,
    description: e.description.filter(|s| !s.is_empty()),
    display_name: e.display_name.map(|name| name.pick(language)).filter(|s| !s.is_empty()),
    high_is_good: e.high_is_good,
    icon_id: e.icon_id,
    name: e.name,
    published: e.published,
    stackable: e.stackable,
    unit_id: e.unit_id,
  }
}

async fn seed_dogma_attributes(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdeDogmaAttrEntry> = read_jsonl(path).await?;

  let records: Vec<DogmaAttributeMeta> = entries
    .into_iter()
    .map(|e| build_dogma_attribute(e, language))
    .collect();

  sde::upsert_many_dogma_attributes(db, &records)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_abyssal_module_stats(db: &Database, path: &Path) -> Result<(), String> {
  let entries: Vec<SdeDynamicEntry> = read_jsonl(path).await?;

  let mut records: Vec<AbyssalModuleStat> = Vec::new();
  for entry in &entries {
    for mapping in &entry.input_output_mapping {
      for bounds in &entry.attribute_ids {
        records.push(AbyssalModuleStat::new(
          i64::from(mapping.resulting_type),
          i64::from(bounds.attribute_id),
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

async fn seed_races(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdeRaceEntry> = read_jsonl(path).await?;

  let records: Vec<Race> = entries
    .into_iter()
    .map(|e| {
      let name = e.name.map(|n| n.pick(language)).unwrap_or_default();
      Race::new(e.id, 0, name.clone(), name)
    })
    .collect();

  for race in &records {
    sde::upsert_race(db, race).await.map_err(|e| e.to_string())?;
  }
  Ok(())
}

async fn seed_bloodlines(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdeBloodlineEntry> = read_jsonl(path).await?;

  let records: Vec<Bloodline> = entries
    .into_iter()
    .map(|e| {
      let name = e.name.map(|n| n.pick(language)).unwrap_or_default();
      Bloodline::new(
        e.id,
        i64::from(e.corporation_id),
        i64::from(e.race_id),
        e.charisma,
        name.clone(),
        e.intelligence,
        e.memory,
        name,
        e.perception,
        e.willpower,
      )
    })
    .collect();

  for bloodline in &records {
    sde::upsert_bloodline(db, bloodline).await.map_err(|e| e.to_string())?;
  }
  Ok(())
}

async fn seed_factions(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdeFactionEntry> = read_jsonl(path).await?;

  let records: Vec<Faction> = entries
    .into_iter()
    .map(|e| {
      let name = e.name.map(|n| n.pick(language)).unwrap_or_default();
      let mut m = Faction::new(e.id, name, false, e.size_factor, 0, 0);
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

async fn seed_certificates(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdeCertEntry> = read_jsonl(path).await?;

  let mut certificates: Vec<Certificate> = Vec::with_capacity(entries.len());
  let mut skills: Vec<CertificateSkill> = Vec::new();

  for e in entries {
    for lvl in &e.skill_types {
      let levels = build_cert_skill_levels(lvl);
      skills.push(CertificateSkill {
        advanced: i64::from(levels[2]),
        basic: i64::from(levels[0]),
        certificate_id: e.id,
        elite: i64::from(levels[3]),
        improved: i64::from(levels[1]),
        skill_id: i64::from(lvl.skill_id),
      });
    }
    certificates.push(Certificate {
      description: e.description.map(|d| d.pick(language)),
      grade: 1,
      id: e.id,
      name: e.name.pick(language),
    });
  }

  skills::certificate_upsert_many(db, &certificates, &skills)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_masteries(db: &Database, path: &Path) -> Result<(), String> {
  let entries: Vec<SdeMasteryEntry> = read_jsonl(path).await?;
  let records = build_mastery_rows(entries);

  if records.is_empty() {
    return Ok(());
  }

  skills::mastery_upsert_many(db, &records)
    .await
    .map_err(|e| e.to_string())
}

fn build_mastery_rows(entries: Vec<SdeMasteryEntry>) -> Vec<ShipMastery> {
  let mut records: Vec<ShipMastery> = Vec::new();
  for entry in entries {
    for tier in entry.tiers {
      if !(0..=4).contains(&tier.tier) {
        continue;
      }
      for certificate_id in tier.certificate_ids {
        records.push(ShipMastery {
          certificate_id,
          ship_type_id: entry.ship_type_id,
          tier: tier.tier + 1,
        });
      }
    }
  }
  records
}

async fn seed_agent_types(db: &Database, path: &Path) -> Result<(), String> {
  let entries: Vec<SdeAgentTypeEntry> = read_jsonl(path).await?;

  let records: Vec<AgentType> = entries
    .into_iter()
    .map(|e| AgentType {
      id: e.id,
      name: e.name,
    })
    .collect();

  sde::upsert_many_agent_types(db, &records)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_npc_corporation_divisions(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdeNpcCorporationDivisionEntry> = read_jsonl(path).await?;

  let records: Vec<NpcCorporationDivision> = entries
    .into_iter()
    .map(|e| NpcCorporationDivision {
      id: e.id,
      name: e.name.map(|n| n.pick(language)).unwrap_or_default(),
    })
    .collect();

  sde::upsert_many_npc_corporation_divisions(db, &records)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_npc_corporations(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdeNpcCorporationEntry> = read_jsonl(path).await?;

  let records: Vec<SeedCorporation> = entries
    .into_iter()
    .map(|e| SeedCorporation {
      faction_id: e.faction_id,
      home_station_id: e.station_id,
      id: e.id,
      name: e.name.map(|n| n.pick(language)).unwrap_or_default(),
      ticker: e.ticker_name.unwrap_or_default(),
    })
    .collect();

  org::upsert_many_seed_corporations(db, &records)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_regions(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdeRegionEntry> = read_jsonl(path).await?;

  let records: Vec<Region> = entries
    .into_iter()
    .map(|e| Region {
      description: None,
      id: e.id,
      name: e.name.map(|n| n.pick(language)).unwrap_or_default(),
    })
    .collect();

  sde::upsert_many_regions(db, &records).await.map_err(|e| e.to_string())
}

async fn seed_constellations(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdeConstellationEntry> = read_jsonl(path).await?;

  let records: Vec<Constellation> = entries
    .into_iter()
    .map(|e| Constellation {
      id: e.id,
      name: e.name.map(|n| n.pick(language)).unwrap_or_default(),
      position_x: e.position.x,
      position_y: e.position.y,
      position_z: e.position.z,
      region_id: e.region_id,
    })
    .collect();

  sde::upsert_many_constellations(db, &records)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_solar_systems(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdeSolarSystemEntry> = read_jsonl(path).await?;

  let records: Vec<SolarSystem> = entries
    .into_iter()
    .map(|e| SolarSystem {
      constellation_id: e.constellation_id,
      id: e.id,
      name: e.name.map(|n| n.pick(language)).unwrap_or_default(),
      position_x: e.position.x,
      position_y: e.position.y,
      position_z: e.position.z,
      security_class: e.security_class,
      security_status: e.security_status,
      star_id: e.star_id,
    })
    .collect();

  sde::upsert_many_solar_systems(db, &records)
    .await
    .map_err(|e| e.to_string())
}

async fn seed_npc_stations(db: &Database, r: &Path, language: Language) -> Result<(), String> {
  let stations: Vec<SdeNpcStationEntry> = read_jsonl(&r.join("npcStations.jsonl")).await?;
  let planets: HashMap<i64, SdeMapPlanetEntry> = read_jsonl::<SdeMapPlanetEntry>(&r.join("mapPlanets.jsonl"))
    .await?
    .into_iter()
    .map(|p| (p.id, p))
    .collect();
  let moons: HashMap<i64, SdeMapMoonEntry> = read_jsonl::<SdeMapMoonEntry>(&r.join("mapMoons.jsonl"))
    .await?
    .into_iter()
    .map(|m| (m.id, m))
    .collect();
  let operations: HashMap<i64, SdeStationOperationEntry> =
    read_jsonl::<SdeStationOperationEntry>(&r.join("stationOperations.jsonl"))
      .await?
      .into_iter()
      .map(|o| (o.id, o))
      .collect();
  let systems = sde::solar_system_names(db).await.map_err(|e| e.to_string())?;
  let corporations = org::corporation_names(db).await.map_err(|e| e.to_string())?;

  let records: Vec<Station> = stations
    .into_iter()
    .map(|e| {
      let orbit_name = derive_orbit_name(e.orbit_id, &planets, &moons, &systems);
      let corporation_name = e.owner_id.and_then(|owner| corporations.get(&owner)).cloned();
      let operation_name = e
        .operation_id
        .and_then(|op| operations.get(&op))
        .and_then(|op| op.operation_name.clone())
        .map(|name| name.pick(language));
      let name = derive_station_name(
        orbit_name.as_deref(),
        corporation_name.as_deref(),
        operation_name.as_deref().filter(|_| e.use_operation_name),
      );

      Station {
        id: e.id,
        max_dockable_ship_volume: 0.0,
        name,
        office_rental_cost: 0.0,
        owner: e.owner_id,
        position_x: e.position.x,
        position_y: e.position.y,
        position_z: e.position.z,
        race_id: None,
        reprocessing_efficiency: e.reprocessing_efficiency,
        reprocessing_stations_take: e.reprocessing_stations_take,
        services: "[]".to_owned(),
        system_id: e.solar_system_id,
        type_id: e.type_id,
      }
    })
    .collect();

  sde::seed_many_stations(db, &records).await.map_err(|e| e.to_string())
}

async fn seed_moons(db: &Database, r: &Path) -> Result<(), String> {
  let moons: HashMap<i64, SdeMapMoonEntry> = read_jsonl::<SdeMapMoonEntry>(&r.join("mapMoons.jsonl"))
    .await?
    .into_iter()
    .map(|m| (m.id, m))
    .collect();
  let planets: HashMap<i64, SdeMapPlanetEntry> = read_jsonl::<SdeMapPlanetEntry>(&r.join("mapPlanets.jsonl"))
    .await?
    .into_iter()
    .map(|p| (p.id, p))
    .collect();
  let systems = sde::solar_system_names(db).await.map_err(|e| e.to_string())?;

  let records: Vec<Moon> = moons
    .values()
    .filter_map(|e| {
      let solar_system_id = e
        .solar_system_id
        .or_else(|| planets.get(&e.orbit_id).map(|planet| planet.solar_system_id))?;
      let name = derive_orbit_name(Some(e.id), &planets, &moons, &systems)?;
      let position = e.position.as_ref();

      Some(Moon {
        id: e.id,
        name,
        orbit_index: Some(i64::from(e.orbit_index)),
        planet_id: Some(e.orbit_id),
        position_x: position.map(|p| p.x).unwrap_or_default(),
        position_y: position.map(|p| p.y).unwrap_or_default(),
        position_z: position.map(|p| p.z).unwrap_or_default(),
        radius: e.radius,
        solar_system_id,
        type_id: e.type_id,
      })
    })
    .collect();

  sde::upsert_many_moons(db, &records).await.map_err(|e| e.to_string())
}

async fn seed_npc_agents(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdeNpcCharacterEntry> = read_jsonl(path).await?;

  let mut agents: Vec<NpcAgent> = Vec::new();
  let mut skills: Vec<NpcAgentSkill> = Vec::new();

  for entry in entries {
    let Some(agent) = entry.agent else {
      continue;
    };

    for skill in &entry.skills {
      skills.push(NpcAgentSkill {
        agent_id: entry.id,
        skill_type_id: skill.type_id,
      });
    }

    agents.push(NpcAgent {
      agent_type_id: agent.agent_type_id,
      corporation_id: entry.corporation_id,
      division_id: agent.division_id,
      id: entry.id,
      is_locator: i32::from(agent.is_locator),
      level: agent.level,
      location_id: entry.location_id,
      name: entry.name.map(|n| n.pick(language)).unwrap_or_default(),
    });
  }

  sde::seed_many_npc_agents(db, &agents, &skills)
    .await
    .map_err(|e| e.to_string())
}

fn blueprint_activity_id(name: &str) -> Option<i64> {
  match name {
    "manufacturing" => Some(1),
    "research_time" => Some(3),
    "research_material" => Some(4),
    "copying" => Some(5),
    "invention" => Some(8),
    "reaction" => Some(11),
    _ => None,
  }
}

/// Returns `true` for manufacturing (1) and reaction (11) — the only activities that produce items
/// and therefore need time and run-cap meta rows.
fn is_build_activity(activity_id: i64) -> bool {
  activity_id == 1 || activity_id == 11
}

fn build_blueprint_rows(
  entries: Vec<SdeBlueprintEntry>,
) -> (
  Vec<BlueprintActivityRow>,
  Vec<BlueprintActivityRow>,
  Vec<BlueprintActivityMetaRow>,
) {
  let mut products: Vec<BlueprintActivityRow> = Vec::new();
  let mut materials: Vec<BlueprintActivityRow> = Vec::new();
  let mut meta: Vec<BlueprintActivityMetaRow> = Vec::new();

  for entry in entries {
    let blueprint_type_id = entry.id;
    for (activity_name, activity) in entry.activities {
      let Some(activity_id) = blueprint_activity_id(&activity_name) else {
        continue;
      };

      if let (true, Some(time)) = (is_build_activity(activity_id), activity.time) {
        meta.push(BlueprintActivityMetaRow {
          activity_id,
          blueprint_type_id,
          max_production_limit: entry.max_production_limit,
          time,
        });
      }

      for product in activity.products {
        products.push(BlueprintActivityRow {
          blueprint_type_id,
          activity_id,
          type_id: product.type_id,
          quantity: product.quantity,
        });
      }

      for material in activity.materials {
        materials.push(BlueprintActivityRow {
          blueprint_type_id,
          activity_id,
          type_id: material.type_id,
          quantity: material.quantity,
        });
      }
    }
  }

  (products, materials, meta)
}

async fn seed_blueprints(db: &Database, path: &Path) -> Result<(), String> {
  let entries: Vec<SdeBlueprintEntry> = read_jsonl(path).await?;
  let (products, materials, meta) = build_blueprint_rows(entries);

  insert_blueprint_rows(db, "blueprint_activity_products", "product_type_id", &products).await?;
  insert_blueprint_rows(db, "blueprint_activity_materials", "material_type_id", &materials).await?;
  insert_blueprint_meta_rows(db, &meta).await?;

  Ok(())
}

async fn insert_blueprint_rows(
  db: &Database,
  table: &str,
  type_column: &str,
  rows: &[BlueprintActivityRow],
) -> Result<(), String> {
  if rows.is_empty() {
    return Ok(());
  }

  let mut tx = db.writer().begin().await.map_err(|e| e.to_string())?;

  for chunk in rows.chunks(SQLITE_MAX_BIND_PARAMS / 4) {
    let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(format!(
      "INSERT INTO {table} (blueprint_type_id, activity_id, {type_column}, quantity) "
    ));
    builder.push_values(chunk, |mut b, row| {
      b.push_bind(row.blueprint_type_id)
        .push_bind(row.activity_id)
        .push_bind(row.type_id)
        .push_bind(row.quantity);
    });
    builder.push(format!(
      " ON CONFLICT(blueprint_type_id, activity_id, {type_column}) DO UPDATE SET quantity = excluded.quantity"
    ));
    builder.build().execute(&mut *tx).await.map_err(|e| e.to_string())?;
  }

  tx.commit().await.map_err(|e| e.to_string())
}

async fn insert_blueprint_meta_rows(db: &Database, rows: &[BlueprintActivityMetaRow]) -> Result<(), String> {
  if rows.is_empty() {
    return Ok(());
  }

  let mut tx = db.writer().begin().await.map_err(|e| e.to_string())?;

  for chunk in rows.chunks(SQLITE_MAX_BIND_PARAMS / 4) {
    let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
      "INSERT INTO blueprint_activity_meta (blueprint_type_id, activity_id, time, max_production_limit) ",
    );
    builder.push_values(chunk, |mut b, row| {
      b.push_bind(row.blueprint_type_id)
        .push_bind(row.activity_id)
        .push_bind(row.time)
        .push_bind(row.max_production_limit);
    });
    builder.push(
      " ON CONFLICT(blueprint_type_id, activity_id) DO UPDATE SET time = excluded.time, \
      max_production_limit = excluded.max_production_limit",
    );
    builder.build().execute(&mut *tx).await.map_err(|e| e.to_string())?;
  }

  tx.commit().await.map_err(|e| e.to_string())
}

fn build_type_material_rows(entries: Vec<SdeTypeMaterialEntry>) -> Vec<TypeMaterial> {
  let mut rows: Vec<TypeMaterial> = Vec::new();
  for entry in entries {
    for material in entry.materials {
      rows.push(TypeMaterial {
        material_type_id: material.material_type_id,
        quantity: material.quantity,
        type_id: entry.id,
      });
    }
  }
  rows
}

async fn seed_type_materials(db: &Database, path: &Path) -> Result<(), String> {
  let entries: Vec<SdeTypeMaterialEntry> = read_jsonl(path).await?;
  let rows = build_type_material_rows(entries);

  sde::seed_many_type_materials(db, &rows)
    .await
    .map_err(|e| e.to_string())
}

fn build_planet_schematic_rows(
  entries: Vec<SdePlanetSchematicEntry>,
  language: Language,
) -> (Vec<sde::PlanetSchematic>, Vec<sde::PlanetSchematicType>) {
  let mut schematics: Vec<sde::PlanetSchematic> = Vec::new();
  let mut types: Vec<sde::PlanetSchematicType> = Vec::new();

  for entry in entries {
    schematics.push(sde::PlanetSchematic {
      cycle_time: entry.cycle_time,
      id: entry.id,
      name: entry.name.map(|n| n.pick(language)).unwrap_or_default(),
    });

    for entry_type in entry.types {
      types.push(sde::PlanetSchematicType {
        is_input: entry_type.is_input,
        quantity: entry_type.quantity,
        schematic_id: entry.id,
        type_id: entry_type.type_id,
      });
    }
  }

  (schematics, types)
}

async fn seed_planet_schematics(db: &Database, path: &Path, language: Language) -> Result<(), String> {
  let entries: Vec<SdePlanetSchematicEntry> = read_jsonl(path).await?;
  let (schematics, types) = build_planet_schematic_rows(entries, language);

  sde::seed_many_planet_schematics(db, &schematics, &types)
    .await
    .map_err(|e| e.to_string())
}

fn derive_orbit_name(
  orbit_id: Option<i64>,
  planets: &HashMap<i64, SdeMapPlanetEntry>,
  moons: &HashMap<i64, SdeMapMoonEntry>,
  systems: &HashMap<i64, String>,
) -> Option<String> {
  let orbit_id = orbit_id?;

  if let Some(planet) = planets.get(&orbit_id) {
    let system_name = systems.get(&planet.solar_system_id)?;
    return Some(format!("{system_name} {}", roman_numeral(planet.celestial_index)));
  }

  if let Some(moon) = moons.get(&orbit_id) {
    let planet = planets.get(&moon.orbit_id)?;
    let system_name = systems.get(&planet.solar_system_id)?;
    let planet_name = format!("{system_name} {}", roman_numeral(planet.celestial_index));
    return Some(format!("{planet_name} - Moon {}", moon.orbit_index));
  }

  None
}

fn derive_station_name(
  orbit_name: Option<&str>,
  corporation_name: Option<&str>,
  operation_name: Option<&str>,
) -> String {
  let orbit = orbit_name.unwrap_or("");
  let corporation = corporation_name.unwrap_or("");

  match operation_name {
    Some(operation) if !operation.is_empty() => format!("{orbit} - {corporation} {operation}"),
    _ => format!("{orbit} - {corporation}"),
  }
}

fn roman_numeral(value: i32) -> String {
  const NUMERALS: [(i32, &str); 13] = [
    (1000, "M"),
    (900, "CM"),
    (500, "D"),
    (400, "CD"),
    (100, "C"),
    (90, "XC"),
    (50, "L"),
    (40, "XL"),
    (10, "X"),
    (9, "IX"),
    (5, "V"),
    (4, "IV"),
    (1, "I"),
  ];

  if value <= 0 {
    return value.to_string();
  }

  let mut remaining = value;
  let mut result = String::new();
  for (amount, numeral) in NUMERALS {
    while remaining >= amount {
      result.push_str(numeral);
      remaining -= amount;
    }
  }
  result
}

fn composite_version(sde_build: &str, language: Language) -> String {
  format!(
    "{}+pod-{}+seed-{}+lang-{}",
    sde_build,
    env!("CARGO_PKG_VERSION"),
    SEED_FORMAT_REVISION,
    language.sde_code()
  )
}

pub fn sde_version_path() -> Option<PathBuf> {
  Some(dir_spec::state_home()?.join(config::APP_DIR).join("sde_version"))
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

pub fn synced_language_path() -> Option<PathBuf> {
  Some(dir_spec::state_home()?.join(config::APP_DIR).join("synced_language"))
}

// True only when a marker is present AND records a different language than the one configured. An
// absent marker (first run, or an upgrade from a pre-i18n build) is treated as already matching, so
// a pilot who has never picked a language sees no forced re-fetch. See ADR-0041 section 3.
pub fn language_switched(marker: Option<Language>, configured: Language) -> bool {
  marker.is_some_and(|synced| synced != configured)
}

pub fn read_synced_language(path: &Path) -> Option<Language> {
  let contents = std::fs::read_to_string(path).ok()?;
  Language::from_code(contents.trim())
}

pub fn write_synced_language(path: &Path, language: Language) {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).ok();
  }
  std::fs::write(path, language.esi_code()).ok();
}

fn clamp_skill_level(v: i32) -> u8 {
  v.clamp(0, 5) as u8
}

fn build_cert_skill_levels(lvl: &SdeCertSkill) -> [u8; 4] {
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

  mod build_type_material_rows {
    use pretty_assertions::assert_eq;

    use super::*;

    fn parse(jsonl: &str) -> Vec<SdeTypeMaterialEntry> {
      jsonl.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    #[test]
    fn it_flattens_each_type_into_its_material_rows() {
      let entries = parse(
        r#"{"_key": 18, "materials": [{"materialTypeID": 34, "quantity": 175}, {"materialTypeID": 36, "quantity": 70}]}"#,
      );

      let mut rows = build_type_material_rows(entries);
      rows.sort_by_key(|row| row.material_type_id);

      assert_eq!(
        rows,
        vec![
          TypeMaterial {
            material_type_id: 34,
            quantity: 175,
            type_id: 18,
          },
          TypeMaterial {
            material_type_id: 36,
            quantity: 70,
            type_id: 18,
          },
        ]
      );
    }

    #[test]
    fn it_ignores_randomized_materials() {
      let entries = parse(
        r#"{"_key": 90283, "randomizedMaterials": [{"materialTypeID": 34, "quantityMax": 496800, "quantityMin": 368000}]}"#,
      );

      let rows = build_type_material_rows(entries);

      assert!(rows.is_empty());
    }
  }

  mod build_planet_schematic_rows {
    use pretty_assertions::assert_eq;

    use super::*;

    fn parse(jsonl: &str) -> Vec<SdePlanetSchematicEntry> {
      jsonl.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    #[test]
    fn it_reads_the_schematic_name_and_cycle_time() {
      let entries = parse(r#"{"_key": 65, "cycleTime": 1800, "name": {"en": "Water"}, "types": []}"#);

      let (schematics, _types) = build_planet_schematic_rows(entries, Language::EnUs);

      assert_eq!(
        schematics,
        vec![sde::PlanetSchematic {
          cycle_time: 1800,
          id: 65,
          name: "Water".to_owned(),
        }]
      );
    }

    #[test]
    fn it_flattens_inputs_and_outputs_with_the_is_input_flag() {
      let entries = parse(
        r#"{"_key": 65, "cycleTime": 1800, "name": {"en": "Water"}, "types": [{"_key": 2309, "isInput": true, "quantity": 3000}, {"_key": 2401, "isInput": false, "quantity": 20}]}"#,
      );

      let (_schematics, mut types) = build_planet_schematic_rows(entries, Language::EnUs);
      types.sort_by_key(|row| row.type_id);

      assert_eq!(
        types,
        vec![
          sde::PlanetSchematicType {
            is_input: true,
            quantity: 3000,
            schematic_id: 65,
            type_id: 2309,
          },
          sde::PlanetSchematicType {
            is_input: false,
            quantity: 20,
            schematic_id: 65,
            type_id: 2401,
          },
        ]
      );
    }

    #[test]
    fn it_defaults_a_missing_name_to_empty() {
      let entries = parse(r#"{"_key": 65, "cycleTime": 1800, "types": []}"#);

      let (schematics, _types) = build_planet_schematic_rows(entries, Language::EnUs);

      assert_eq!(schematics.first().unwrap().name, "");
    }
  }

  mod build_blueprint_rows {
    use pretty_assertions::assert_eq;

    use super::*;

    fn parse(jsonl: &str) -> Vec<SdeBlueprintEntry> {
      jsonl.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    #[test]
    fn it_captures_activity_time_and_max_production_limit() {
      let entries = parse(
        r#"{"_key": 939, "activities": {"manufacturing": {"products": [{"typeID": 587, "quantity": 1}], "time": 600}}, "maxProductionLimit": 300}"#,
      );

      let (_products, _materials, meta) = build_blueprint_rows(entries);

      assert_eq!(
        meta,
        vec![BlueprintActivityMetaRow {
          activity_id: 1,
          blueprint_type_id: 939,
          max_production_limit: 300,
          time: 600,
        }]
      );
    }

    #[test]
    fn it_defaults_a_missing_max_production_limit_to_zero() {
      let entries = parse(
        r#"{"_key": 939, "activities": {"reaction": {"products": [{"typeID": 16640, "quantity": 200}], "time": 3600}}}"#,
      );

      let (_products, _materials, meta) = build_blueprint_rows(entries);

      assert_eq!(
        meta,
        vec![BlueprintActivityMetaRow {
          activity_id: 11,
          blueprint_type_id: 939,
          max_production_limit: 0,
          time: 3600,
        }]
      );
    }

    #[test]
    fn it_omits_meta_for_an_activity_without_a_time() {
      let entries = parse(
        r#"{"_key": 939, "activities": {"manufacturing": {"products": [{"typeID": 587, "quantity": 1}]}}, "maxProductionLimit": 300}"#,
      );

      let (_products, _materials, meta) = build_blueprint_rows(entries);

      assert!(meta.is_empty());
    }

    #[test]
    fn it_skips_unknown_activity_names() {
      let entries = parse(
        r#"{"_key": 1, "activities": {"mystery": {"materials": [{"typeID": 34, "quantity": 1}], "products": [{"typeID": 587, "quantity": 1}]}}}"#,
      );

      let (products, materials, meta) = build_blueprint_rows(entries);

      assert!(products.is_empty());
      assert!(materials.is_empty());
      assert!(meta.is_empty());
    }

    #[test]
    fn it_splits_products_and_materials_with_mapped_activity_ids() {
      let entries = parse(
        r#"{"_key": 939, "activities": {"manufacturing": {"materials": [{"typeID": 34, "quantity": 32}], "products": [{"typeID": 587, "quantity": 1}]}}}"#,
      );

      let (products, materials, _meta) = build_blueprint_rows(entries);

      assert_eq!(
        products,
        vec![BlueprintActivityRow {
          blueprint_type_id: 939,
          activity_id: 1,
          type_id: 587,
          quantity: 1,
        }]
      );
      assert_eq!(
        materials,
        vec![BlueprintActivityRow {
          blueprint_type_id: 939,
          activity_id: 1,
          type_id: 34,
          quantity: 32,
        }]
      );
    }
  }

  mod build_cert_skill_levels {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_levels_into_the_zero_to_five_range() {
      let lvl = SdeCertSkill {
        basic: -3,
        improved: 0,
        advanced: 5,
        elite: 9,
        skill_id: 3300,
      };

      assert_eq!(build_cert_skill_levels(&lvl), [0, 0, 5, 5]);
    }

    #[test]
    fn it_maps_each_grade_level_in_order() {
      let lvl = SdeCertSkill {
        basic: 1,
        improved: 2,
        advanced: 3,
        elite: 4,
        skill_id: 3300,
      };

      assert_eq!(build_cert_skill_levels(&lvl), [1, 2, 3, 4]);
    }
  }

  mod build_dogma_attribute {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_entry(display_name: Option<&str>, description: Option<&str>) -> SdeDogmaAttrEntry {
      SdeDogmaAttrEntry {
        id: 48,
        name: "cpuOutput".to_owned(),
        display_name: display_name.map(|d| LocalizedString {
          en: Some(d.to_owned()),
          ..LocalizedString::default()
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
    fn it_drops_empty_display_name_and_description_to_none() {
      let model = build_dogma_attribute(make_entry(Some(""), Some("")), Language::EnUs);

      assert_eq!(model.display_name(), &None);
      assert_eq!(model.description(), &None);
    }

    #[test]
    fn it_maps_the_localized_display_name_to_english() {
      let model = build_dogma_attribute(make_entry(Some("CPU Output"), None), Language::EnUs);

      assert_eq!(model.attribute_id(), 48);
      assert_eq!(model.name(), "cpuOutput");
      assert_eq!(model.display_name().as_deref(), Some("CPU Output"));
      assert_eq!(model.high_is_good(), true);
      assert_eq!(model.unit_id(), Some(101));
    }
  }

  mod build_item_type {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_type_entry(description: Option<&str>) -> SdeTypeEntry {
      SdeTypeEntry {
        id: 34,
        name: LocalizedString {
          en: Some("Tritanium".to_owned()),
          ..LocalizedString::default()
        },
        description: description.map(|d| LocalizedString {
          en: Some(d.to_owned()),
          ..LocalizedString::default()
        }),
        group_id: 18,
        market_group_id: None,
        capacity: None,
        volume: None,
        portion_size: None,
        radius: None,
        published: true,
        icon_id: None,
      }
    }

    #[test]
    fn it_defaults_a_missing_description_to_empty_string_never_null() {
      let model = build_item_type(make_type_entry(None), None, Language::EnUs);

      assert_eq!(model.description(), &Some(String::new()));
    }

    #[test]
    fn it_leaves_packaged_volume_null() {
      let model = build_item_type(make_type_entry(None), None, Language::EnUs);

      assert_eq!(model.packaged_volume(), None);
    }

    #[test]
    fn it_preserves_a_present_description() {
      let model = build_item_type(make_type_entry(Some("The most common ore")), None, Language::EnUs);

      assert_eq!(model.description(), &Some("The most common ore".to_owned()));
    }
  }

  mod build_mastery_rows {
    use pretty_assertions::assert_eq;

    use super::*;

    fn parse(jsonl: &str) -> Vec<SdeMasteryEntry> {
      jsonl.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    #[test]
    fn it_accepts_tier_index_4_storing_it_as_5() {
      let entries = parse(r#"{"_key": 1234, "_value": [{"_key": 4, "_value": [100]}]}"#);

      let rows = build_mastery_rows(entries);

      assert_eq!(rows.len(), 1);
      assert_eq!(rows[0].tier, 5);
    }

    #[test]
    fn it_extracts_ship_tier_and_cert_ids() {
      let entries = parse(r#"{"_key": 1234, "_value": [{"_key": 2, "_value": [100, 200]}]}"#);

      let rows = build_mastery_rows(entries);

      assert_eq!(
        rows,
        vec![
          ShipMastery {
            certificate_id: 100,
            ship_type_id: 1234,
            tier: 3,
          },
          ShipMastery {
            certificate_id: 200,
            ship_type_id: 1234,
            tier: 3,
          },
        ]
      );
    }

    #[test]
    fn it_returns_empty_for_no_entries() {
      let rows = build_mastery_rows(Vec::new());

      assert!(rows.is_empty());
    }

    #[test]
    fn it_returns_empty_for_empty_cert_lists() {
      let entries = parse(r#"{"_key": 1234, "_value": [{"_key": 1, "_value": []}]}"#);

      let rows = build_mastery_rows(entries);

      assert!(rows.is_empty());
    }

    #[test]
    fn it_skips_tier_index_5_and_above() {
      let entries = parse(r#"{"_key": 1234, "_value": [{"_key": 5, "_value": [100]}]}"#);

      let rows = build_mastery_rows(entries);

      assert!(rows.is_empty());
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
        id: 3300,
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
    fn it_returns_none_when_a_required_attribute_is_missing() {
      let d = dogma(&[(275, 1.0), (180, 167.0)]);

      assert!(build_skill_metadata(3300, Some(&d)).is_none());
    }

    #[test]
    fn it_returns_none_when_no_dogma_exists() {
      assert!(build_skill_metadata(3300, None).is_none());
    }

    #[test]
    fn it_rounds_fractional_dogma_values() {
      let d = dogma(&[(275, 2.6), (180, 167.4), (181, 165.5)]);

      let result = build_skill_metadata(3300, Some(&d)).unwrap();

      assert_eq!(result.rank(), 3);
      assert_eq!(result.primary_attribute(), 167);
      assert_eq!(result.secondary_attribute(), 166);
    }
  }

  mod composite_version {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_differs_when_sde_build_differs() {
      let a = composite_version("20240101.1", Language::En);
      let b = composite_version("20240102.1", Language::En);

      assert_ne!(a, b);
    }

    #[test]
    fn it_embeds_the_sde_build_pod_version_seed_revision_and_language() {
      let result = composite_version("20240101.1", Language::Fr);

      assert_eq!(
        result,
        format!(
          "20240101.1+pod-{}+seed-{}+lang-fr",
          env!("CARGO_PKG_VERSION"),
          SEED_FORMAT_REVISION
        )
      );
    }

    #[test]
    fn it_differs_across_languages() {
      let en = composite_version("20240101.1", Language::En);
      let fr = composite_version("20240101.1", Language::Fr);

      assert_ne!(en, fr);
    }

    #[test]
    fn it_is_stable_for_the_same_language() {
      let first = composite_version("20240101.1", Language::Ja);
      let second = composite_version("20240101.1", Language::Ja);

      assert_eq!(first, second);
    }

    #[test]
    fn it_collapses_en_us_to_en() {
      let en = composite_version("20240101.1", Language::En);
      let en_us = composite_version("20240101.1", Language::EnUs);

      assert_eq!(en, en_us);
    }
  }

  mod derive_orbit_name {
    use pretty_assertions::assert_eq;

    use super::*;

    fn fixture() -> (
      HashMap<i64, SdeMapPlanetEntry>,
      HashMap<i64, SdeMapMoonEntry>,
      HashMap<i64, String>,
    ) {
      let planets = HashMap::from([(
        40009082,
        SdeMapPlanetEntry {
          celestial_index: 4,
          id: 40009082,
          solar_system_id: 30000142,
        },
      )]);
      let moons = HashMap::from([(
        40009087,
        SdeMapMoonEntry {
          id: 40009087,
          orbit_id: 40009082,
          orbit_index: 4,
          position: None,
          radius: None,
          solar_system_id: None,
          type_id: None,
        },
      )]);
      let systems = HashMap::from([(30000142, "Jita".to_owned())]);
      (planets, moons, systems)
    }

    #[test]
    fn it_names_a_moon_with_an_arabic_orbit_index_under_its_parent_planet() {
      let (planets, moons, systems) = fixture();

      let name = derive_orbit_name(Some(40009087), &planets, &moons, &systems);

      assert_eq!(name.as_deref(), Some("Jita IV - Moon 4"));
    }

    #[test]
    fn it_names_a_planet_with_a_roman_celestial_index() {
      let (planets, moons, systems) = fixture();

      let name = derive_orbit_name(Some(40009082), &planets, &moons, &systems);

      assert_eq!(name.as_deref(), Some("Jita IV"));
    }

    #[test]
    fn it_returns_none_for_an_unknown_orbit() {
      let (planets, moons, systems) = fixture();

      assert_eq!(derive_orbit_name(Some(999), &planets, &moons, &systems), None);
      assert_eq!(derive_orbit_name(None, &planets, &moons, &systems), None);
    }
  }

  mod derive_station_name {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_appends_the_operation_name_when_present() {
      let name = derive_station_name(Some("Jita IV - Moon 4"), Some("Caldari Navy"), Some("Assembly Plant"));

      assert_eq!(name, "Jita IV - Moon 4 - Caldari Navy Assembly Plant");
    }

    #[test]
    fn it_omits_the_operation_name_when_absent() {
      let name = derive_station_name(Some("Tanoo IV"), Some("Amarr Navy"), None);

      assert_eq!(name, "Tanoo IV - Amarr Navy");
    }
  }

  mod read_jsonl {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_parses_one_record_per_line_skipping_blank_lines() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("agentTypes.jsonl");
      tokio::fs::write(
        &path,
        "{\"_key\": 2, \"name\": \"BasicAgent\"}\n\n{\"_key\": 4, \"name\": \"ResearchAgent\"}\n",
      )
      .await
      .unwrap();

      let entries: Vec<SdeAgentTypeEntry> = super::super::read_jsonl(&path).await.unwrap();

      assert_eq!(entries.len(), 2);
      assert_eq!(entries[0].id, 2);
      assert_eq!(entries[0].name, "BasicAgent");
      assert_eq!(entries[1].id, 4);
    }

    #[tokio::test]
    async fn it_reports_a_parse_error_with_the_path() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("agentTypes.jsonl");
      tokio::fs::write(&path, "not-json\n").await.unwrap();

      let result: Result<Vec<SdeAgentTypeEntry>, String> = super::super::read_jsonl(&path).await;

      assert!(result.err().unwrap().contains("agentTypes.jsonl"));
    }

    #[tokio::test]
    async fn it_reports_a_read_error_for_a_missing_file() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("missing.jsonl");

      let result: Result<Vec<SdeAgentTypeEntry>, String> = super::super::read_jsonl(&path).await;

      assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_returns_an_empty_vec_for_an_empty_file() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("agentTypes.jsonl");
      tokio::fs::write(&path, "").await.unwrap();

      let entries: Vec<SdeAgentTypeEntry> = super::super::read_jsonl(&path).await.unwrap();

      assert!(entries.is_empty());
    }
  }

  mod pick {
    use pretty_assertions::assert_eq;

    use super::*;

    fn parse(json: &str) -> LocalizedString {
      serde_json::from_str(json).unwrap()
    }

    #[test]
    fn it_falls_back_to_empty_when_neither_the_language_nor_en_is_present() {
      let localized = parse(r#"{"ja": "モジュール"}"#);

      assert_eq!(localized.pick(Language::De), String::new());
    }

    #[test]
    fn it_falls_back_to_en_when_the_chosen_language_is_blank() {
      let localized = parse(r#"{"de": "", "en": "Module"}"#);

      assert_eq!(localized.pick(Language::De), "Module");
    }

    #[test]
    fn it_falls_back_to_en_when_the_chosen_language_is_missing() {
      let localized = parse(r#"{"en": "Module", "fr": "Module"}"#);

      assert_eq!(localized.pick(Language::De), "Module");
    }

    #[test]
    fn it_keeps_en_for_an_en_request() {
      let localized = parse(r#"{"de": "Modul", "en": "Module"}"#);

      assert_eq!(localized.pick(Language::En), "Module");
    }

    #[test]
    fn it_picks_the_chosen_language_when_present() {
      let localized = parse(r#"{"de": "Modul", "en": "Module", "fr": "Module"}"#);

      assert_eq!(localized.pick(Language::De), "Modul");
    }

    #[test]
    fn it_resolves_en_us_through_the_en_field() {
      let localized = parse(r#"{"de": "Modul", "en": "Module"}"#);

      assert_eq!(localized.pick(Language::EnUs), "Module");
    }
  }

  mod roman_numeral {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_converts_celestial_indices_to_roman_numerals() {
      assert_eq!(roman_numeral(1), "I");
      assert_eq!(roman_numeral(4), "IV");
      assert_eq!(roman_numeral(9), "IX");
      assert_eq!(roman_numeral(14), "XIV");
      assert_eq!(roman_numeral(29), "XXIX");
    }

    #[test]
    fn it_falls_back_to_arabic_for_non_positive_values() {
      assert_eq!(roman_numeral(0), "0");
      assert_eq!(roman_numeral(-3), "-3");
    }
  }

  mod sde_is_current {
    use super::*;

    #[test]
    fn it_reports_current_when_the_marker_matches_the_composite() {
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("sde_version");
      let composite = composite_version("20240101.1", Language::En);
      write_stored_sde_version(&marker, &composite);

      assert!(sde_is_current(Some(&marker), Some(&composite)));
    }

    #[test]
    fn it_reports_stale_for_a_versionless_build() {
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("sde_version");
      write_stored_sde_version(&marker, &composite_version("20240101.1", Language::En));

      assert!(!sde_is_current(Some(&marker), None));
    }

    #[test]
    fn it_reports_stale_when_the_marker_differs() {
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("sde_version");
      write_stored_sde_version(&marker, &composite_version("20240101.1", Language::En));

      assert!(!sde_is_current(
        Some(&marker),
        Some(&composite_version("20240102.1", Language::En))
      ));
    }

    #[test]
    fn it_reports_stale_when_only_the_language_differs() {
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("sde_version");
      write_stored_sde_version(&marker, &composite_version("20240101.1", Language::En));

      assert!(!sde_is_current(
        Some(&marker),
        Some(&composite_version("20240101.1", Language::Fr))
      ));
    }

    #[test]
    fn it_reports_stale_when_the_marker_is_absent() {
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("sde_version");

      assert!(!sde_is_current(
        Some(&marker),
        Some("20240101.1+pod-0.5.0+seed-2+lang-en")
      ));
    }
  }

  mod language_switched {
    use super::*;

    #[test]
    fn it_reports_no_switch_when_the_marker_is_absent() {
      assert!(!language_switched(None, Language::Fr));
    }

    #[test]
    fn it_reports_no_switch_when_the_marker_matches_the_configured_language() {
      assert!(!language_switched(Some(Language::De), Language::De));
    }

    #[test]
    fn it_reports_a_switch_when_the_marker_differs_from_the_configured_language() {
      assert!(language_switched(Some(Language::En), Language::Fr));
    }
  }

  mod read_synced_language {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_the_written_language() {
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("synced_language");
      write_synced_language(&marker, Language::Ja);

      assert_eq!(read_synced_language(&marker), Some(Language::Ja));
    }

    #[test]
    fn it_round_trips_en_us_through_the_esi_code() {
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("synced_language");
      write_synced_language(&marker, Language::EnUs);

      assert_eq!(read_synced_language(&marker), Some(Language::EnUs));
    }

    #[test]
    fn it_returns_none_when_the_marker_is_absent() {
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("synced_language");

      assert_eq!(read_synced_language(&marker), None);
    }

    #[test]
    fn it_returns_none_for_an_unrecognized_code() {
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("synced_language");
      std::fs::write(&marker, "xx").unwrap();

      assert_eq!(read_synced_language(&marker), None);
    }

    #[test]
    fn it_trims_surrounding_whitespace_before_parsing() {
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("synced_language");
      std::fs::write(&marker, "  fr\n").unwrap();

      assert_eq!(read_synced_language(&marker), Some(Language::Fr));
    }
  }

  mod seed_abyssal_module_stats {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, repo::assets};

    const FIXTURE: &str = r#"{"_key": 47405, "attributeIDs": [{"_key": 6, "max": 1.4, "min": 0.6}, {"_key": 30, "max": 1.1, "min": 0.9}], "inputOutputMapping": [{"applicableTypes": [12058], "resultingType": 47408}, {"applicableTypes": [12060], "resultingType": 47410}]}"#;

    #[tokio::test]
    async fn it_is_idempotent_across_reseed() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("dynamicItemAttributes.jsonl");
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

    #[tokio::test]
    async fn it_seeds_one_bound_row_per_resulting_type_and_attribute() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("dynamicItemAttributes.jsonl");
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
  }

  mod seed_all_tables {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{
      self,
      repo::{sde, skills},
    };

    async fn write_jsonl(dir: &Path, name: &str, body: &str) {
      tokio::fs::write(dir.join(name), body).await.unwrap();
    }

    async fn write_full_fixture(dir: &Path) {
      write_jsonl(
        dir,
        "categories.jsonl",
        "{\"_key\": 16, \"name\": {\"en\": \"Skill\"}, \"published\": true}\n\
        {\"_key\": 25, \"name\": {\"en\": \"Ship\"}, \"published\": true}\n",
      )
      .await;
      write_jsonl(
        dir,
        "groups.jsonl",
        "{\"_key\": 255, \"categoryID\": 16, \"name\": {\"en\": \"Gunnery\"}, \"published\": true}\n\
        {\"_key\": 25, \"categoryID\": 25, \"name\": {\"en\": \"Frigate\"}, \"published\": true}\n",
      )
      .await;
      write_jsonl(
        dir,
        "marketGroups.jsonl",
        "{\"_key\": 9, \"name\": {\"en\": \"Ships\"}, \"hasTypes\": false}\n",
      )
      .await;
      write_jsonl(
        dir,
        "types.jsonl",
        "{\"_key\": 3300, \"groupID\": 255, \"name\": {\"en\": \"Gunnery\"}, \"published\": true}\n\
        {\"_key\": 596, \"groupID\": 25, \"name\": {\"en\": \"Impairor\"}, \"published\": true}\n",
      )
      .await;
      write_jsonl(
        dir,
        "typeDogma.jsonl",
        "{\"_key\": 3300, \"dogmaAttributes\": [{\"attributeID\": 275, \"value\": 1.0}, \
        {\"attributeID\": 180, \"value\": 167.0}, {\"attributeID\": 181, \"value\": 166.0}]}\n",
      )
      .await;
      write_jsonl(
        dir,
        "dogmaAttributes.jsonl",
        "{\"_key\": 4, \"name\": \"mass\", \"displayName\": {\"en\": \"Mass\"}, \"defaultValue\": 0.0, \
        \"highIsGood\": false, \"unitID\": 2, \"iconID\": 100, \"published\": true, \"stackable\": true}\n",
      )
      .await;
      write_jsonl(dir, "races.jsonl", "{\"_key\": 1, \"name\": {\"en\": \"Caldari\"}}\n").await;
      write_jsonl(
        dir,
        "bloodlines.jsonl",
        "{\"_key\": 5, \"name\": {\"en\": \"Deteis\"}, \"raceID\": 1, \"corporationID\": 1000035}\n",
      )
      .await;
      write_jsonl(
        dir,
        "factions.jsonl",
        "{\"_key\": 500001, \"name\": {\"en\": \"Caldari State\"}, \"solarSystemID\": 30000145}\n",
      )
      .await;
      write_jsonl(
        dir,
        "certificates.jsonl",
        "{\"_key\": 100, \"name\": {\"en\": \"Core Fitting\"}, \"skillTypes\": [{\"_key\": 3300, \"basic\": 1}]}\n",
      )
      .await;
      write_jsonl(
        dir,
        "masteries.jsonl",
        "{\"_key\": 596, \"_value\": [{\"_key\": 1, \"_value\": [100]}]}\n",
      )
      .await;
      write_jsonl(
        dir,
        "npcCorporations.jsonl",
        "{\"_key\": 1000035, \"name\": {\"en\": \"Caldari Navy\"}, \"factionID\": 500001, \
        \"stationID\": 60000004, \"tickerName\": \"CN\"}\n",
      )
      .await;
      write_jsonl(
        dir,
        "mapRegions.jsonl",
        "{\"_key\": 10000002, \"name\": {\"en\": \"The Forge\"}}\n",
      )
      .await;
      write_jsonl(
        dir,
        "mapConstellations.jsonl",
        "{\"_key\": 20000020, \"name\": {\"en\": \"Kimotoro\"}, \"regionID\": 10000002, \
        \"position\": {\"x\": 0.0, \"y\": 0.0, \"z\": 0.0}}\n",
      )
      .await;
      write_jsonl(
        dir,
        "mapSolarSystems.jsonl",
        "{\"_key\": 30000142, \"name\": {\"en\": \"Jita\"}, \"constellationID\": 20000020, \
        \"securityStatus\": 0.95, \"securityClass\": \"B\", \"position\": {\"x\": 0.0, \"y\": 0.0, \"z\": 0.0}}\n",
      )
      .await;
      write_jsonl(
        dir,
        "mapPlanets.jsonl",
        "{\"_key\": 40009082, \"celestialIndex\": 4, \"solarSystemID\": 30000142}\n",
      )
      .await;
      write_jsonl(
        dir,
        "mapMoons.jsonl",
        "{\"_key\": 40009087, \"orbitID\": 40009082, \"orbitIndex\": 4}\n",
      )
      .await;
      write_jsonl(
        dir,
        "stationOperations.jsonl",
        "{\"_key\": 14, \"operationName\": {\"en\": \"Assembly Plant\"}}\n",
      )
      .await;
      write_jsonl(
        dir,
        "npcStations.jsonl",
        "{\"_key\": 60000004, \"operationID\": 14, \"orbitID\": 40009087, \"ownerID\": 1000035, \
        \"solarSystemID\": 30000142, \"typeID\": 596, \"useOperationName\": true, \
        \"reprocessingEfficiency\": 0.5, \"reprocessingStationsTake\": 0.05, \
        \"position\": {\"x\": 1.0, \"y\": 2.0, \"z\": 3.0}}\n",
      )
      .await;
      write_jsonl(
        dir,
        "agentTypes.jsonl",
        "{\"_key\": 2, \"name\": \"BasicAgent\"}\n{\"_key\": 4, \"name\": \"ResearchAgent\"}\n",
      )
      .await;
      write_jsonl(
        dir,
        "npcCorporationDivisions.jsonl",
        "{\"_key\": 22, \"name\": {\"en\": \"Distribution\"}}\n",
      )
      .await;
      write_jsonl(
        dir,
        "npcCharacters.jsonl",
        "{\"_key\": 3008416, \"agent\": {\"agentTypeID\": 2, \"divisionID\": 22, \"isLocator\": false, \"level\": 1}, \
        \"corporationID\": 1000035, \"locationID\": 60000004, \"name\": {\"en\": \"Antaken Kamola\"}, \
        \"skills\": [{\"typeID\": 3300}]}\n",
      )
      .await;
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

      seed_all_tables(&db, &mut tx, tmp.path(), Language::EnUs).await.unwrap();

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

      let region: String = sqlx::query_scalar("SELECT name FROM regions WHERE id = 10000002")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(region, "The Forge");

      let station: String = sqlx::query_scalar("SELECT name FROM stations WHERE id = 60000004")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(station, "Jita IV - Moon 4 - Caldari Navy Assembly Plant");

      let agent: String = sqlx::query_scalar("SELECT name FROM npc_agents WHERE id = 3008416")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(agent, "Antaken Kamola");
    }

    #[tokio::test]
    async fn it_silently_skips_the_optional_files_when_absent() {
      let tmp = tempfile::tempdir().unwrap();
      write_full_fixture(tmp.path()).await;
      tokio::fs::remove_file(tmp.path().join("certificates.jsonl"))
        .await
        .unwrap();
      tokio::fs::remove_file(tmp.path().join("masteries.jsonl"))
        .await
        .unwrap();
      let db = store::open_test().await.unwrap();
      let (mut tx, _rx) = channel();

      seed_all_tables(&db, &mut tx, tmp.path(), Language::EnUs).await.unwrap();

      assert!(skills::by_ids(&db, &[100]).await.unwrap().is_empty());
      assert!(skills::for_ship(&db, 596).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn seed_if_stale_backfills_dogma_attributes_when_current_but_unseeded() {
      let tmp = tempfile::tempdir().unwrap();
      write_full_fixture(tmp.path()).await;
      let marker = tmp.path().join("sde_version");
      write_stored_sde_version(&marker, &composite_version("20240101.1", Language::EnUs));
      let db = store::open_test().await.unwrap();
      let (mut tx, _rx) = channel();

      seed_if_stale_at(
        &db,
        &mut tx,
        tmp.path(),
        Some("20240101.1"),
        Some(&marker),
        Language::EnUs,
      )
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

      seed_if_stale_at(&db, &mut tx, tmp.path(), None, Some(&marker), Language::EnUs)
        .await
        .unwrap();

      assert!(sde::get_race(&db, 1).await.unwrap().is_some());
      assert!(!marker.exists());
    }

    #[tokio::test]
    async fn seed_if_stale_skips_seeding_when_the_stored_version_matches() {
      let tmp = tempfile::tempdir().unwrap();
      write_full_fixture(tmp.path()).await;
      let marker = tmp.path().join("sde_version");
      write_stored_sde_version(&marker, &composite_version("20240101.1", Language::EnUs));
      let db = store::open_test().await.unwrap();
      let (mut tx, _rx) = channel();

      seed_if_stale_at(
        &db,
        &mut tx,
        tmp.path(),
        Some("20240101.1"),
        Some(&marker),
        Language::EnUs,
      )
      .await
      .unwrap();

      assert!(sde::get_race(&db, 1).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn seed_if_stale_re_seeds_when_only_the_language_changes() {
      let tmp = tempfile::tempdir().unwrap();
      write_full_fixture(tmp.path()).await;
      let marker = tmp.path().join("sde_version");
      write_stored_sde_version(&marker, &composite_version("20240101.1", Language::En));
      let db = store::open_test().await.unwrap();
      let (mut tx, _rx) = channel();

      seed_if_stale_at(
        &db,
        &mut tx,
        tmp.path(),
        Some("20240101.1"),
        Some(&marker),
        Language::Fr,
      )
      .await
      .unwrap();

      assert!(sde::get_race(&db, 1).await.unwrap().is_some());
      assert_eq!(
        read_stored_sde_version(&marker),
        Some(composite_version("20240101.1", Language::Fr))
      );
    }

    #[tokio::test]
    async fn seed_if_stale_writes_the_version_marker_when_stale() {
      let tmp = tempfile::tempdir().unwrap();
      write_full_fixture(tmp.path()).await;
      let marker = tmp.path().join("sde_version");
      let db = store::open_test().await.unwrap();
      let (mut tx, _rx) = channel();

      seed_if_stale_at(
        &db,
        &mut tx,
        tmp.path(),
        Some("20240101.1"),
        Some(&marker),
        Language::EnUs,
      )
      .await
      .unwrap();

      assert_eq!(
        read_stored_sde_version(&marker),
        Some(composite_version("20240101.1", Language::EnUs))
      );
      assert!(sde::get_race(&db, 1).await.unwrap().is_some());
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
    async fn it_leaves_ship_type_id_null() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("bloodlines.jsonl");
      tokio::fs::write(
        &path,
        "{\"_key\": 5, \"name\": {\"en\": \"Deteis\"}, \"raceID\": 1, \"corporationID\": 1000035}\n",
      )
      .await
      .unwrap();
      let db = store::open_test().await.unwrap();
      seed_parent_race(&db).await;

      seed_bloodlines(&db, &path, Language::EnUs).await.unwrap();

      let deteis = sde::get_bloodline(&db, 5).await.unwrap().unwrap();
      assert_eq!(deteis.ship_type_id, None);
    }

    #[tokio::test]
    async fn it_seeds_bloodlines_with_their_attributes() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("bloodlines.jsonl");
      tokio::fs::write(
        &path,
        "{\"_key\": 5, \"name\": {\"en\": \"Deteis\"}, \"raceID\": 1, \"corporationID\": 1000035, \
        \"charisma\": 6, \"intelligence\": 9, \"memory\": 7, \"perception\": 4, \"willpower\": 4}\n",
      )
      .await
      .unwrap();
      let db = store::open_test().await.unwrap();
      seed_parent_race(&db).await;

      seed_bloodlines(&db, &path, Language::EnUs).await.unwrap();

      let deteis = sde::get_bloodline(&db, 5).await.unwrap().unwrap();
      assert_eq!(deteis.name, "Deteis");
      assert_eq!(deteis.race_id, 1);
      assert_eq!(deteis.ship_type_id, None);
      assert_eq!(deteis.charisma, 6);
    }
  }

  mod seed_blueprints {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    const FIXTURE: &str = "{\"_key\": 939, \"activities\": {\"manufacturing\": {\"materials\": \
[{\"typeID\": 34, \"quantity\": 32}, {\"typeID\": 35, \"quantity\": 6}], \"products\": \
[{\"typeID\": 587, \"quantity\": 1}], \"time\": 600}, \"copying\": {\"time\": 480}}, \
\"blueprintTypeID\": 939, \"maxProductionLimit\": 300}\n\
{\"_key\": 46167, \"activities\": {\"reaction\": {\"materials\": [{\"typeID\": 16633, \"quantity\": 100}], \
\"products\": [{\"typeID\": 16640, \"quantity\": 200}], \"time\": 3600}}, \"blueprintTypeID\": 46167}\n";

    async fn seed_fixture() -> Database {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("blueprints.jsonl");
      tokio::fs::write(&path, FIXTURE).await.unwrap();
      let db = store::open_test().await.unwrap();
      super::super::seed_blueprints(&db, &path).await.unwrap();
      db
    }

    #[tokio::test]
    async fn it_is_idempotent_across_reseed() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("blueprints.jsonl");
      tokio::fs::write(&path, FIXTURE).await.unwrap();
      let db = store::open_test().await.unwrap();

      super::super::seed_blueprints(&db, &path).await.unwrap();
      super::super::seed_blueprints(&db, &path).await.unwrap();

      let products: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blueprint_activity_products")
        .fetch_one(&db.0)
        .await
        .unwrap();
      let materials: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blueprint_activity_materials")
        .fetch_one(&db.0)
        .await
        .unwrap();

      assert_eq!(products, 2);
      assert_eq!(materials, 3);
    }

    #[tokio::test]
    async fn it_records_reaction_products_under_activity_eleven() {
      let db = seed_fixture().await;

      let quantity: i64 = sqlx::query_scalar(
        "SELECT quantity FROM blueprint_activity_products \
        WHERE blueprint_type_id = 46167 AND activity_id = 11 AND product_type_id = 16640",
      )
      .fetch_one(&db.0)
      .await
      .unwrap();

      assert_eq!(quantity, 200);
    }

    #[tokio::test]
    async fn it_seeds_a_manufacturing_product_row() {
      let db = seed_fixture().await;

      let quantity: i64 = sqlx::query_scalar(
        "SELECT quantity FROM blueprint_activity_products \
        WHERE blueprint_type_id = 939 AND activity_id = 1 AND product_type_id = 587",
      )
      .fetch_one(&db.0)
      .await
      .unwrap();

      assert_eq!(quantity, 1);
    }

    #[tokio::test]
    async fn it_seeds_manufacturing_activity_meta() {
      let db = seed_fixture().await;

      let row: (i64, i64) = sqlx::query_as(
        "SELECT time, max_production_limit FROM blueprint_activity_meta \
        WHERE blueprint_type_id = 939 AND activity_id = 1",
      )
      .fetch_one(&db.0)
      .await
      .unwrap();

      assert_eq!(row, (600, 300));
    }

    #[tokio::test]
    async fn it_seeds_manufacturing_material_rows() {
      let db = seed_fixture().await;

      let quantity: i64 = sqlx::query_scalar(
        "SELECT quantity FROM blueprint_activity_materials \
        WHERE blueprint_type_id = 939 AND activity_id = 1 AND material_type_id = 34",
      )
      .fetch_one(&db.0)
      .await
      .unwrap();

      assert_eq!(quantity, 32);
    }

    #[tokio::test]
    async fn it_seeds_reaction_activity_meta() {
      let db = seed_fixture().await;

      let row: (i64, i64) = sqlx::query_as(
        "SELECT time, max_production_limit FROM blueprint_activity_meta \
        WHERE blueprint_type_id = 46167 AND activity_id = 11",
      )
      .fetch_one(&db.0)
      .await
      .unwrap();

      assert_eq!(row, (3600, 0));
    }

    #[tokio::test]
    async fn it_skips_meta_for_non_build_activities() {
      let db = seed_fixture().await;

      let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM blueprint_activity_meta WHERE activity_id NOT IN (1, 11)")
          .fetch_one(&db.0)
          .await
          .unwrap();

      assert_eq!(count, 0);
    }
  }

  mod seed_catalog {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    async fn write(dir: &Path, name: &str, body: &str) {
      tokio::fs::write(dir.join(name), body).await.unwrap();
    }

    async fn seed_item_type(db: &Database, id: i64) {
      sqlx::query("INSERT OR IGNORE INTO item_categories (id, name, published) VALUES (16, 'Skill', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query("INSERT OR IGNORE INTO item_groups (id, category_id, name, published) VALUES (255, 16, 'G', 1)")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query(
        "INSERT OR IGNORE INTO item_types (id, group_id, description, name, published) VALUES (?, 255, '', 'Skill', 1)",
      )
      .bind(id)
      .execute(db.writer())
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_is_idempotent_across_reseed() {
      let tmp = tempfile::tempdir().unwrap();
      write(
        tmp.path(),
        "agentTypes.jsonl",
        "{\"_key\": 2, \"name\": \"BasicAgent\"}\n",
      )
      .await;
      let db = store::open_test().await.unwrap();

      seed_agent_types(&db, &tmp.path().join("agentTypes.jsonl"))
        .await
        .unwrap();
      seed_agent_types(&db, &tmp.path().join("agentTypes.jsonl"))
        .await
        .unwrap();

      let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_types")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn it_seeds_agent_types_and_divisions() {
      let tmp = tempfile::tempdir().unwrap();
      write(
        tmp.path(),
        "agentTypes.jsonl",
        "{\"_key\": 2, \"name\": \"BasicAgent\"}\n{\"_key\": 4, \"name\": \"ResearchAgent\"}\n",
      )
      .await;
      write(
        tmp.path(),
        "npcCorporationDivisions.jsonl",
        "{\"_key\": 18, \"name\": {\"en\": \"R&D\"}}\n{\"_key\": 22, \"name\": {\"en\": \"Distribution\"}}\n",
      )
      .await;
      let db = store::open_test().await.unwrap();

      seed_agent_types(&db, &tmp.path().join("agentTypes.jsonl"))
        .await
        .unwrap();
      seed_npc_corporation_divisions(&db, &tmp.path().join("npcCorporationDivisions.jsonl"), Language::EnUs)
        .await
        .unwrap();

      let agent_type: String = sqlx::query_scalar("SELECT name FROM agent_types WHERE id = 4")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(agent_type, "ResearchAgent");

      let division: String = sqlx::query_scalar("SELECT name FROM npc_corporation_divisions WHERE id = 18")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(division, "R&D");
    }

    #[tokio::test]
    async fn it_seeds_geo_and_derives_station_names_end_to_end() {
      let tmp = tempfile::tempdir().unwrap();
      let r = tmp.path();
      write(
        r,
        "mapRegions.jsonl",
        "{\"_key\": 10000002, \"name\": {\"en\": \"The Forge\"}}\n",
      )
      .await;
      write(
        r,
        "mapConstellations.jsonl",
        "{\"_key\": 20000020, \"name\": {\"en\": \"Kimotoro\"}, \"regionID\": 10000002, \
        \"position\": {\"x\": 0.0, \"y\": 0.0, \"z\": 0.0}}\n",
      )
      .await;
      write(
        r,
        "mapSolarSystems.jsonl",
        "{\"_key\": 30000142, \"name\": {\"en\": \"Jita\"}, \"constellationID\": 20000020, \
        \"securityStatus\": 0.95, \"securityClass\": \"B\", \"position\": {\"x\": 0.0, \"y\": 0.0, \"z\": 0.0}}\n",
      )
      .await;
      write(
        r,
        "npcCorporations.jsonl",
        "{\"_key\": 1000035, \"name\": {\"en\": \"Caldari Navy\"}, \"factionID\": 500001, \
        \"stationID\": 60003760, \"tickerName\": \"CN\"}\n",
      )
      .await;
      write(
        r,
        "mapPlanets.jsonl",
        "{\"_key\": 40009082, \"celestialIndex\": 4, \"solarSystemID\": 30000142}\n",
      )
      .await;
      write(
        r,
        "mapMoons.jsonl",
        "{\"_key\": 40009087, \"orbitID\": 40009082, \"orbitIndex\": 4}\n",
      )
      .await;
      write(
        r,
        "stationOperations.jsonl",
        "{\"_key\": 14, \"operationName\": {\"en\": \"Assembly Plant\"}}\n",
      )
      .await;
      write(
        r,
        "npcStations.jsonl",
        "{\"_key\": 60003760, \"operationID\": 14, \"orbitID\": 40009087, \"ownerID\": 1000035, \
        \"solarSystemID\": 30000142, \"typeID\": 52678, \"useOperationName\": true, \
        \"reprocessingEfficiency\": 0.5, \"reprocessingStationsTake\": 0.05, \
        \"position\": {\"x\": 1.0, \"y\": 2.0, \"z\": 3.0}}\n",
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_item_type(&db, 52678).await;

      seed_npc_corporations(&db, &r.join("npcCorporations.jsonl"), Language::EnUs)
        .await
        .unwrap();
      seed_regions(&db, &r.join("mapRegions.jsonl"), Language::EnUs)
        .await
        .unwrap();
      seed_constellations(&db, &r.join("mapConstellations.jsonl"), Language::EnUs)
        .await
        .unwrap();
      seed_solar_systems(&db, &r.join("mapSolarSystems.jsonl"), Language::EnUs)
        .await
        .unwrap();
      seed_npc_stations(&db, r, Language::EnUs).await.unwrap();

      let region: String = sqlx::query_scalar("SELECT name FROM regions WHERE id = 10000002")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(region, "The Forge");

      let station: String = sqlx::query_scalar("SELECT name FROM stations WHERE id = 60003760")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(station, "Jita IV - Moon 4 - Caldari Navy Assembly Plant");
    }

    #[tokio::test]
    async fn it_seeds_npc_agents_and_their_skills() {
      let tmp = tempfile::tempdir().unwrap();
      let r = tmp.path();
      write(
        r,
        "npcCharacters.jsonl",
        "{\"_key\": 3008416, \"agent\": {\"agentTypeID\": 2, \"divisionID\": 22, \"isLocator\": false, \
        \"level\": 1}, \"name\": {\"en\": \"Antaken Kamola\"}, \
        \"skills\": [{\"typeID\": 3300}, {\"typeID\": 3301}]}\n\
        {\"_key\": 3008999, \"name\": {\"en\": \"Not An Agent\"}}\n",
      )
      .await;
      let db = store::open_test().await.unwrap();
      seed_item_type(&db, 3300).await;
      seed_item_type(&db, 3301).await;
      sqlx::query("INSERT INTO agent_types (id, name) VALUES (2, 'BasicAgent')")
        .execute(db.writer())
        .await
        .unwrap();
      sqlx::query("INSERT INTO npc_corporation_divisions (id, name) VALUES (22, 'Distribution')")
        .execute(db.writer())
        .await
        .unwrap();

      seed_npc_agents(&db, &r.join("npcCharacters.jsonl"), Language::EnUs)
        .await
        .unwrap();

      let agents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM npc_agents")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(agents, 1);

      let (name, level): (String, i64) = sqlx::query_as("SELECT name, level FROM npc_agents WHERE id = 3008416")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(name, "Antaken Kamola");
      assert_eq!(level, 1);

      let skills: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM npc_agent_skills WHERE agent_id = 3008416")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(skills, 2);
    }

    #[tokio::test]
    async fn it_upserts_npc_corporations_without_clobbering_esi_columns() {
      let tmp = tempfile::tempdir().unwrap();
      write(
        tmp.path(),
        "npcCorporations.jsonl",
        "{\"_key\": 1000035, \"name\": {\"en\": \"Caldari Navy\"}, \"factionID\": 500001, \
        \"stationID\": 60000001, \"tickerName\": \"CN\"}\n",
      )
      .await;
      let db = store::open_test().await.unwrap();
      sqlx::query(
        "INSERT INTO corporations (id, ceo_id, creator_id, member_count, name, tax_rate, ticker) \
        VALUES (1000035, 42, 7, 99, 'Stale', 0.1, 'OLD')",
      )
      .execute(db.writer())
      .await
      .unwrap();

      seed_npc_corporations(&db, &tmp.path().join("npcCorporations.jsonl"), Language::EnUs)
        .await
        .unwrap();

      let (name, ticker, faction, ceo, members): (String, String, i64, i64, i64) =
        sqlx::query_as("SELECT name, ticker, faction_id, ceo_id, member_count FROM corporations WHERE id = 1000035")
          .fetch_one(&db.0)
          .await
          .unwrap();
      assert_eq!(name, "Caldari Navy");
      assert_eq!(ticker, "CN");
      assert_eq!(faction, 500_001);

      assert_eq!(ceo, 42);
      assert_eq!(members, 99);
    }
  }

  mod seed_certificates {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, repo::skills};

    async fn seed_parent_skills(db: &Database, dir: &Path, ids: &[i64]) {
      tokio::fs::write(
        dir.join("categories.jsonl"),
        "{\"_key\": 16, \"name\": {\"en\": \"Skill\"}}\n",
      )
      .await
      .unwrap();
      tokio::fs::write(
        dir.join("groups.jsonl"),
        "{\"_key\": 255, \"categoryID\": 16, \"name\": {\"en\": \"Gunnery\"}}\n",
      )
      .await
      .unwrap();
      let types: String = ids
        .iter()
        .map(|id| format!("{{\"_key\": {id}, \"groupID\": 255, \"name\": {{\"en\": \"Skill\"}}}}\n"))
        .collect();
      tokio::fs::write(dir.join("types.jsonl"), types).await.unwrap();
      tokio::fs::write(dir.join("typeDogma.jsonl"), "").await.unwrap();

      seed_categories(db, &dir.join("categories.jsonl"), Language::EnUs)
        .await
        .unwrap();
      seed_groups(db, &dir.join("groups.jsonl"), Language::EnUs)
        .await
        .unwrap();
      seed_types(
        db,
        &dir.join("types.jsonl"),
        &dir.join("typeDogma.jsonl"),
        &dir.join("groups.jsonl"),
        Language::EnUs,
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_stores_the_constant_grade_one() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("certificates.jsonl");
      tokio::fs::write(
        &path,
        "{\"_key\": 1001, \"name\": {\"en\": \"NoGrade\"}}\n\
        {\"_key\": 1002, \"name\": {\"en\": \"AlsoNoGrade\"}}\n",
      )
      .await
      .unwrap();
      let db = store::open_test().await.unwrap();

      seed_certificates(&db, &path, Language::EnUs).await.unwrap();

      let certs = skills::by_ids(&db, &[1001, 1002]).await.unwrap();
      let by_id = |id: i64| certs.iter().find(|c| c.id() == id).unwrap();
      assert_eq!(by_id(1001).grade(), 1);
      assert_eq!(by_id(1002).grade(), 1);
    }

    #[tokio::test]
    async fn it_seeds_certificates_with_their_per_skill_levels_ignoring_standard() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("certificates.jsonl");
      tokio::fs::write(
        &path,
        "{\"_key\": 1001, \"name\": {\"en\": \"Core Fitting\"}, \"description\": {\"en\": \"Basic fitting\"}, \
        \"skillTypes\": [{\"_key\": 3300, \"basic\": 1, \"improved\": 3, \"advanced\": 4, \"elite\": 5, \
        \"standard\": 2}, {\"_key\": 3301, \"basic\": 2}]}\n",
      )
      .await
      .unwrap();
      let db = store::open_test().await.unwrap();
      seed_parent_skills(&db, tmp.path(), &[3300, 3301]).await;

      seed_certificates(&db, &path, Language::EnUs).await.unwrap();

      let cert = skills::by_ids(&db, &[1001]).await.unwrap();
      assert_eq!(cert.len(), 1);
      assert_eq!(cert[0].grade(), 1);
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
  }

  mod seed_factions {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, repo::sde};

    #[tokio::test]
    async fn it_seeds_factions_with_solar_system_and_constant_unique_flag() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("factions.jsonl");
      tokio::fs::write(
        &path,
        "{\"_key\": 500001, \"name\": {\"en\": \"Caldari State\"}, \"sizeFactor\": 5.5, \
        \"solarSystemID\": 30000145}\n\
        {\"_key\": 500024, \"name\": {\"en\": \"Generic\"}}\n",
      )
      .await
      .unwrap();
      let db = store::open_test().await.unwrap();

      seed_factions(&db, &path, Language::EnUs).await.unwrap();

      let state = sde::get_faction(&db, 500_001).await.unwrap().unwrap();
      assert_eq!(state.name, "Caldari State");
      assert_eq!(state.size_factor, 5.5);
      assert_eq!(state.solar_system_id, Some(30_000_145));
      assert_eq!(state.is_unique, 0);
      assert_eq!(state.station_count, 0);

      let generic = sde::get_faction(&db, 500_024).await.unwrap().unwrap();
      assert_eq!(generic.size_factor, 1.0);
      assert_eq!(generic.is_unique, 0);
      assert_eq!(generic.solar_system_id, None);
    }
  }

  mod seed_masteries {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, model::Certificate, repo::skills};

    async fn seed_parents(db: &Database, dir: &Path) {
      tokio::fs::write(
        dir.join("categories.jsonl"),
        "{\"_key\": 25, \"name\": {\"en\": \"Ship\"}}\n",
      )
      .await
      .unwrap();
      tokio::fs::write(
        dir.join("groups.jsonl"),
        "{\"_key\": 25, \"categoryID\": 25, \"name\": {\"en\": \"Frigate\"}}\n",
      )
      .await
      .unwrap();
      tokio::fs::write(
        dir.join("types.jsonl"),
        "{\"_key\": 596, \"groupID\": 25, \"name\": {\"en\": \"Impairor\"}}\n",
      )
      .await
      .unwrap();
      tokio::fs::write(dir.join("typeDogma.jsonl"), "").await.unwrap();
      seed_categories(db, &dir.join("categories.jsonl"), Language::EnUs)
        .await
        .unwrap();
      seed_groups(db, &dir.join("groups.jsonl"), Language::EnUs)
        .await
        .unwrap();
      seed_types(
        db,
        &dir.join("types.jsonl"),
        &dir.join("typeDogma.jsonl"),
        &dir.join("groups.jsonl"),
        Language::EnUs,
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
    async fn it_is_a_noop_when_the_file_has_no_usable_rows() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("masteries.jsonl");
      tokio::fs::write(&path, "{\"_key\": 596, \"_value\": [{\"_key\": 1, \"_value\": []}]}\n")
        .await
        .unwrap();
      let db = store::open_test().await.unwrap();

      seed_masteries(&db, &path).await.unwrap();

      let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ship_masteries")
        .fetch_one(&db.0)
        .await
        .unwrap();
      assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn it_seeds_one_mastery_row_per_cert_at_each_tier() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("masteries.jsonl");
      tokio::fs::write(
        &path,
        "{\"_key\": 596, \"_value\": [{\"_key\": 1, \"_value\": [100]}]}\n",
      )
      .await
      .unwrap();
      let db = store::open_test().await.unwrap();
      seed_parents(&db, tmp.path()).await;

      seed_masteries(&db, &path).await.unwrap();

      let masteries = skills::for_ship(&db, 596).await.unwrap();
      assert_eq!(masteries.len(), 1);
      assert_eq!(masteries[0].certificate_id(), 100);
      assert_eq!(masteries[0].tier(), 2);
    }
  }

  mod seed_races {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, repo::sde};

    #[tokio::test]
    async fn it_seeds_races_with_a_constant_zero_alliance() {
      let tmp = tempfile::tempdir().unwrap();
      let path = tmp.path().join("races.jsonl");
      tokio::fs::write(
        &path,
        "{\"_key\": 1, \"name\": {\"en\": \"Caldari\"}}\n{\"_key\": 4, \"name\": {\"en\": \"Jove\"}}\n",
      )
      .await
      .unwrap();
      let db = store::open_test().await.unwrap();

      seed_races(&db, &path, Language::EnUs).await.unwrap();

      let caldari = sde::get_race(&db, 1).await.unwrap().unwrap();
      assert_eq!(caldari.name(), "Caldari");
      assert_eq!(caldari.alliance_id(), 0);

      let jove = sde::get_race(&db, 4).await.unwrap().unwrap();
      assert_eq!(jove.alliance_id(), 0);
    }
  }

  mod seed_types {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store;

    async fn write_jsonl(dir: &Path, name: &str, body: &str) {
      tokio::fs::write(dir.join(name), body).await.unwrap();
    }

    async fn write_fixture(dir: &Path) {
      write_jsonl(
        dir,
        "categories.jsonl",
        "{\"_key\": 16, \"name\": {\"en\": \"Skill\"}, \"published\": true}\n\
        {\"_key\": 4, \"name\": {\"en\": \"Material\"}, \"published\": true}\n",
      )
      .await;
      write_jsonl(
        dir,
        "groups.jsonl",
        "{\"_key\": 255, \"categoryID\": 16, \"name\": {\"en\": \"Gunnery\"}, \"published\": true}\n\
        {\"_key\": 18, \"categoryID\": 4, \"name\": {\"en\": \"Mineral\"}, \"published\": true}\n",
      )
      .await;
      write_jsonl(
        dir,
        "types.jsonl",
        "{\"_key\": 3300, \"groupID\": 255, \"name\": {\"en\": \"Gunnery\"}, \"published\": true}\n\
        {\"_key\": 3301, \"groupID\": 255, \"name\": {\"en\": \"Small Hybrid Turret\"}, \"published\": true}\n\
        {\"_key\": 3302, \"groupID\": 255, \"name\": {\"en\": \"Retired Skill\"}, \"published\": false}\n\
        {\"_key\": 34, \"groupID\": 18, \"name\": {\"en\": \"Tritanium\"}, \"published\": true}\n",
      )
      .await;
      write_jsonl(
        dir,
        "typeDogma.jsonl",
        "{\"_key\": 3300, \"dogmaAttributes\": [{\"attributeID\": 275, \"value\": 1.0}, \
        {\"attributeID\": 180, \"value\": 167.0}, {\"attributeID\": 181, \"value\": 166.0}]}\n\
        {\"_key\": 3301, \"dogmaAttributes\": [{\"attributeID\": 275, \"value\": 2.0}, \
        {\"attributeID\": 180, \"value\": 167.0}, {\"attributeID\": 181, \"value\": 168.0}]}\n",
      )
      .await;
    }

    async fn run_seed_types(dir: &Path) -> Database {
      let db = store::open_test().await.unwrap();
      seed_categories(&db, &dir.join("categories.jsonl"), Language::EnUs)
        .await
        .unwrap();
      seed_groups(&db, &dir.join("groups.jsonl"), Language::EnUs)
        .await
        .unwrap();
      seed_types(
        &db,
        &dir.join("types.jsonl"),
        &dir.join("typeDogma.jsonl"),
        &dir.join("groups.jsonl"),
        Language::EnUs,
      )
      .await
      .unwrap();
      db
    }

    #[tokio::test]
    async fn it_carries_module_skill_requirement_dogma_through_to_the_picker() {
      let tmp = tempfile::tempdir().unwrap();
      write_jsonl(
        tmp.path(),
        "categories.jsonl",
        "{\"_key\": 7, \"name\": {\"en\": \"Module\"}, \"published\": true}\n\
        {\"_key\": 16, \"name\": {\"en\": \"Skill\"}, \"published\": true}\n",
      )
      .await;
      write_jsonl(
        tmp.path(),
        "groups.jsonl",
        "{\"_key\": 55, \"categoryID\": 7, \"name\": {\"en\": \"Projectile Weapon\"}, \"published\": true}\n\
        {\"_key\": 255, \"categoryID\": 16, \"name\": {\"en\": \"Gunnery\"}, \"published\": true}\n",
      )
      .await;
      write_jsonl(
        tmp.path(),
        "types.jsonl",
        "{\"_key\": 3300, \"groupID\": 255, \"name\": {\"en\": \"Gunnery\"}, \"published\": true}\n\
        {\"_key\": 2929, \"groupID\": 55, \"name\": {\"en\": \"200mm AutoCannon I\"}, \"published\": true}\n",
      )
      .await;
      write_jsonl(
        tmp.path(),
        "typeDogma.jsonl",
        "{\"_key\": 2929, \"dogmaAttributes\": [{\"attributeID\": 182, \"value\": 3300.0}, \
        {\"attributeID\": 277, \"value\": 1.0}]}\n",
      )
      .await;

      let db = run_seed_types(tmp.path()).await;

      let modules = skills::modules_for_picker(&db).await.unwrap();
      assert_eq!(modules.len(), 1);
      assert_eq!(modules[0].id, 2929);
      assert_eq!(modules[0].group_name, "Projectile Weapon");
      assert_eq!(modules[0].skill_requirements, vec![("Gunnery".to_owned(), 1)]);
    }

    #[tokio::test]
    async fn it_is_idempotent_across_reseed() {
      let tmp = tempfile::tempdir().unwrap();
      write_fixture(tmp.path()).await;

      let db = run_seed_types(tmp.path()).await;
      seed_types(
        &db,
        &tmp.path().join("types.jsonl"),
        &tmp.path().join("typeDogma.jsonl"),
        &tmp.path().join("groups.jsonl"),
        Language::EnUs,
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
  }

  mod should_skip_download {
    use super::*;

    #[test]
    fn it_downloads_when_only_the_build_matches_a_stale_composite() {
      let dir = tempfile::tempdir().unwrap();
      let marker = dir.path().join("sde_version");
      std::fs::write(&marker, "12345+pod-0.0.0+seed-0+lang-en").unwrap();

      assert!(!should_skip_download(Some("12345"), Some(&marker), true, Language::En));
    }

    #[test]
    fn it_downloads_when_the_database_is_not_yet_seeded() {
      let dir = tempfile::tempdir().unwrap();
      let marker = dir.path().join("sde_version");
      std::fs::write(&marker, composite_version("12345", Language::En)).unwrap();

      assert!(!should_skip_download(Some("12345"), Some(&marker), false, Language::En));
    }

    #[test]
    fn it_downloads_when_the_probe_returns_nothing() {
      let dir = tempfile::tempdir().unwrap();
      let marker = dir.path().join("sde_version");
      std::fs::write(&marker, composite_version("12345", Language::En)).unwrap();

      assert!(!should_skip_download(None, Some(&marker), true, Language::En));
    }

    #[test]
    fn it_downloads_when_only_the_language_changed() {
      let dir = tempfile::tempdir().unwrap();
      let marker = dir.path().join("sde_version");
      std::fs::write(&marker, composite_version("12345", Language::En)).unwrap();

      assert!(!should_skip_download(Some("12345"), Some(&marker), true, Language::Fr));
    }

    #[test]
    fn it_skips_when_the_marker_matches_the_current_composite() {
      let dir = tempfile::tempdir().unwrap();
      let marker = dir.path().join("sde_version");
      std::fs::write(&marker, composite_version("12345", Language::En)).unwrap();

      assert!(should_skip_download(Some("12345"), Some(&marker), true, Language::En));
    }
  }

  mod language_reseed_roundtrip {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, repo::sde};

    const BUILD: &str = "20240101.1";

    async fn write_localized_fixture(dir: &Path) {
      write_jsonl(
        dir,
        "categories.jsonl",
        "{\"_key\": 25, \"name\": {\"en\": \"Ship\", \"fr\": \"Vaisseau\"}, \"published\": true}\n",
      )
      .await;
      write_jsonl(
        dir,
        "groups.jsonl",
        "{\"_key\": 25, \"categoryID\": 25, \"name\": {\"en\": \"Frigate\", \"fr\": \"Frégate\"}, \
        \"published\": true}\n",
      )
      .await;
      write_jsonl(
        dir,
        "marketGroups.jsonl",
        "{\"_key\": 9, \"name\": {\"en\": \"Ships\", \"fr\": \"Vaisseaux\"}}\n",
      )
      .await;
      write_jsonl(
        dir,
        "types.jsonl",
        "{\"_key\": 596, \"groupID\": 25, \"name\": {\"en\": \"Impairor\", \"fr\": \"Châtieur\"}, \
        \"description\": {\"en\": \"A rookie ship.\", \"fr\": \"Un vaisseau de débutant.\"}, \"published\": true}\n\
        {\"_key\": 597, \"groupID\": 25, \"name\": {\"en\": \"Reaper\"}, \
        \"description\": {\"en\": \"A rookie ship.\"}, \"published\": true}\n",
      )
      .await;
      write_jsonl(dir, "typeDogma.jsonl", "").await;
      write_jsonl(dir, "dogmaAttributes.jsonl", "").await;
      write_jsonl(
        dir,
        "races.jsonl",
        "{\"_key\": 1, \"name\": {\"en\": \"Caldari\", \"fr\": \"Caldari\"}}\n",
      )
      .await;
      write_jsonl(
        dir,
        "bloodlines.jsonl",
        "{\"_key\": 5, \"name\": {\"en\": \"Deteis\"}, \"raceID\": 1, \"corporationID\": 1000035}\n",
      )
      .await;
      write_jsonl(
        dir,
        "factions.jsonl",
        "{\"_key\": 500001, \"name\": {\"en\": \"Caldari State\", \"fr\": \"État Caldari\"}}\n\
        {\"_key\": 500024, \"name\": {\"en\": \"Generic\"}}\n",
      )
      .await;
      write_jsonl(
        dir,
        "npcCorporations.jsonl",
        "{\"_key\": 1000035, \"name\": {\"en\": \"Caldari Navy\", \"fr\": \"Marine Caldari\"}, \
        \"factionID\": 500001, \"tickerName\": \"CN\"}\n",
      )
      .await;
      write_jsonl(
        dir,
        "mapRegions.jsonl",
        "{\"_key\": 10000002, \"name\": {\"en\": \"The Forge\", \"fr\": \"La Forge\"}}\n\
        {\"_key\": 10000003, \"name\": {\"en\": \"Domain\"}}\n\
        {\"_key\": 9999}\n",
      )
      .await;
      write_jsonl(
        dir,
        "mapConstellations.jsonl",
        "{\"_key\": 20000020, \"name\": {\"en\": \"Kimotoro\"}, \"regionID\": 10000002, \
        \"position\": {\"x\": 0.0, \"y\": 0.0, \"z\": 0.0}}\n",
      )
      .await;
      write_jsonl(
        dir,
        "mapSolarSystems.jsonl",
        "{\"_key\": 30000142, \"name\": {\"en\": \"Jita\"}, \"constellationID\": 20000020, \
        \"securityStatus\": 0.95, \"position\": {\"x\": 0.0, \"y\": 0.0, \"z\": 0.0}}\n",
      )
      .await;
      write_jsonl(dir, "mapPlanets.jsonl", "").await;
      write_jsonl(dir, "mapMoons.jsonl", "").await;
      write_jsonl(dir, "stationOperations.jsonl", "").await;
      write_jsonl(dir, "npcStations.jsonl", "").await;
      write_jsonl(dir, "agentTypes.jsonl", "").await;
      write_jsonl(dir, "npcCorporationDivisions.jsonl", "").await;
      write_jsonl(dir, "npcCharacters.jsonl", "").await;
    }

    async fn write_jsonl(dir: &Path, name: &str, body: &str) {
      tokio::fs::write(dir.join(name), body).await.unwrap();
    }

    fn channel() -> (Tx, iced::futures::channel::mpsc::Receiver<Progress>) {
      iced::futures::channel::mpsc::channel(64)
    }

    async fn type_name(db: &Database, id: i64) -> String {
      sqlx::query_scalar("SELECT name FROM item_types WHERE id = ?")
        .bind(id)
        .fetch_one(&db.0)
        .await
        .unwrap()
    }

    async fn type_description(db: &Database, id: i64) -> Option<String> {
      sqlx::query_scalar("SELECT description FROM item_types WHERE id = ?")
        .bind(id)
        .fetch_one(&db.0)
        .await
        .unwrap()
    }

    async fn region_name(db: &Database, id: i64) -> String {
      sqlx::query_scalar("SELECT name FROM regions WHERE id = ?")
        .bind(id)
        .fetch_one(&db.0)
        .await
        .unwrap()
    }

    async fn count(db: &Database, query: &'static str) -> i64 {
      sqlx::query_scalar(query).fetch_one(&db.0).await.unwrap()
    }

    async fn seed_then_reseed(dir: &Path, marker: &Path) -> Database {
      let db = store::open_test().await.unwrap();
      let (mut tx, _rx) = channel();

      seed_if_stale_at(&db, &mut tx, dir, Some(BUILD), Some(marker), Language::En)
        .await
        .unwrap();
      seed_if_stale_at(&db, &mut tx, dir, Some(BUILD), Some(marker), Language::Fr)
        .await
        .unwrap();

      db
    }

    #[tokio::test]
    async fn it_falls_back_to_en_when_the_chosen_language_is_missing() {
      let tmp = tempfile::tempdir().unwrap();
      write_localized_fixture(tmp.path()).await;
      let marker = tmp.path().join("sde_version");

      let db = seed_then_reseed(tmp.path(), &marker).await;

      assert_eq!(type_name(&db, 597).await, "Reaper");
      assert_eq!(type_description(&db, 597).await.as_deref(), Some("A rookie ship."));
      assert_eq!(region_name(&db, 10_000_003).await, "Domain");

      let generic = sde::get_faction(&db, 500_024).await.unwrap().unwrap();
      assert_eq!(generic.name, "Generic");
    }

    #[tokio::test]
    async fn it_persists_an_empty_name_when_every_language_is_missing() {
      let tmp = tempfile::tempdir().unwrap();
      write_localized_fixture(tmp.path()).await;
      let marker = tmp.path().join("sde_version");

      let db = seed_then_reseed(tmp.path(), &marker).await;

      assert_eq!(region_name(&db, 9999).await, "");
    }

    #[tokio::test]
    async fn it_re_seeds_in_place_keeping_ids_and_row_counts() {
      let tmp = tempfile::tempdir().unwrap();
      write_localized_fixture(tmp.path()).await;
      let marker = tmp.path().join("sde_version");
      let db = store::open_test().await.unwrap();
      let (mut tx, _rx) = channel();

      seed_if_stale_at(&db, &mut tx, tmp.path(), Some(BUILD), Some(&marker), Language::En)
        .await
        .unwrap();
      let regions_after_en = count(&db, "SELECT COUNT(*) FROM regions").await;
      let types_after_en = count(&db, "SELECT COUNT(*) FROM item_types").await;
      let factions_after_en = count(&db, "SELECT COUNT(*) FROM factions").await;
      assert_eq!(region_name(&db, 10_000_002).await, "The Forge");

      seed_if_stale_at(&db, &mut tx, tmp.path(), Some(BUILD), Some(&marker), Language::Fr)
        .await
        .unwrap();

      assert_eq!(count(&db, "SELECT COUNT(*) FROM regions").await, regions_after_en);
      assert_eq!(count(&db, "SELECT COUNT(*) FROM item_types").await, types_after_en);
      assert_eq!(count(&db, "SELECT COUNT(*) FROM factions").await, factions_after_en);

      assert!(sde::get_faction(&db, 500_001).await.unwrap().is_some());
      assert!(sde::get_race(&db, 1).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn it_replaces_text_with_the_new_language_where_present() {
      let tmp = tempfile::tempdir().unwrap();
      write_localized_fixture(tmp.path()).await;
      let marker = tmp.path().join("sde_version");

      let db = seed_then_reseed(tmp.path(), &marker).await;

      assert_eq!(type_name(&db, 596).await, "Châtieur");
      assert_eq!(
        type_description(&db, 596).await.as_deref(),
        Some("Un vaisseau de débutant.")
      );
      assert_eq!(region_name(&db, 10_000_002).await, "La Forge");

      let state = sde::get_faction(&db, 500_001).await.unwrap().unwrap();
      assert_eq!(state.name, "État Caldari");
    }

    #[tokio::test]
    async fn it_rewrites_the_marker_so_the_re_seed_is_triggered_not_skipped() {
      let tmp = tempfile::tempdir().unwrap();
      write_localized_fixture(tmp.path()).await;
      let marker = tmp.path().join("sde_version");
      let db = store::open_test().await.unwrap();
      let (mut tx, _rx) = channel();

      seed_if_stale_at(&db, &mut tx, tmp.path(), Some(BUILD), Some(&marker), Language::En)
        .await
        .unwrap();
      assert_eq!(
        read_stored_sde_version(&marker),
        Some(composite_version(BUILD, Language::En))
      );

      seed_if_stale_at(&db, &mut tx, tmp.path(), Some(BUILD), Some(&marker), Language::Fr)
        .await
        .unwrap();

      assert_eq!(
        read_stored_sde_version(&marker),
        Some(composite_version(BUILD, Language::Fr))
      );
      assert_eq!(type_name(&db, 596).await, "Châtieur");
    }
  }
}
