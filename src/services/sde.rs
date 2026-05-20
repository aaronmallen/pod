//! EVE Online Static Data Export (SDE) download and seeding.

use std::{
  collections::HashMap,
  io::{Read as _, Write as _},
  path::{Path, PathBuf},
};

use iced::{Task, futures::SinkExt as _};
use pod_model as models;
use serde::Deserialize;

use crate::services::bootstrap;

/// Kicks off SDE download and seeding, streaming progress messages back to the
/// bootstrap pipeline.
#[tracing::instrument(skip(db))]
pub fn seed(db: pod_db::Repo) -> Task<bootstrap::Message> {
  let (tx, rx) = iced::futures::channel::mpsc::channel(64);
  tokio::spawn(run_seed(db, tx));
  Task::stream(rx)
}

async fn run_seed(db: pod_db::Repo, mut tx: iced::futures::channel::mpsc::Sender<bootstrap::Message>) {
  match do_seed(db, &mut tx).await {
    Ok(db) => {
      let _ = tx.send(bootstrap::Message::SeedingComplete(db)).await;
    }
    Err(e) => {
      let _ = tx.send(bootstrap::Message::Error(format!("SDE seed error: {e}"))).await;
    }
  }
}

#[tracing::instrument(skip(db, tx))]
async fn do_seed(
  db: pod_db::Repo,
  tx: &mut iced::futures::channel::mpsc::Sender<bootstrap::Message>,
) -> Result<pod_db::Repo, String> {
  let esi = pod_esi::Client::builder(crate::ESI_CLIENT_ID)
    .build()
    .map_err(|e| e.to_string())?;

  let tmp = std::env::temp_dir().join("pod_sde");
  tokio::fs::create_dir_all(&tmp).await.map_err(|e| e.to_string())?;

  let zip_path = tmp.join("sde.zip");

  step(tx, "Downloading static data\u{2026}").await;
  esi
    .static_data()
    .download_yaml(&zip_path)
    .await
    .map_err(|e| e.to_string())?;

  step(tx, "Extracting static data\u{2026}").await;
  let extract_dir = tmp.join("extracted");
  tokio::fs::create_dir_all(&extract_dir)
    .await
    .map_err(|e| e.to_string())?;
  extract_zip(&zip_path, &extract_dir).await?;

  let build_version = read_sde_build_version(&extract_dir).await;

  if build_version.as_deref() == read_stored_sde_version().as_deref() && build_version.is_some() {
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    return Ok(db);
  }

  let root = find_sde_root(&extract_dir).await;
  let r = root.as_path();

  db.disable_foreign_keys().await.map_err(|e| e.to_string())?;

  let seed_result: Result<(), String> = async {
    step(tx, "Seeding item categories\u{2026}").await;
    seed_categories(&db, &r.join("categories.yaml")).await?;

    step(tx, "Seeding item groups\u{2026}").await;
    seed_groups(&db, &r.join("groups.yaml")).await?;

    step(tx, "Seeding market groups\u{2026}").await;
    seed_market_groups(&db, &r.join("marketGroups.yaml")).await?;

    step(tx, "Seeding item types\u{2026}").await;
    seed_types(&db, &r.join("types.yaml"), &r.join("typeDogma.yaml")).await?;

    if r.join("certificates.yaml").exists() {
      step(tx, "Seeding certificates\u{2026}").await;
      seed_certificates(&db, &r.join("certificates.yaml")).await?;
    }

    if r.join("masteries.yaml").exists() {
      step(tx, "Seeding ship masteries\u{2026}").await;
      seed_masteries(&db, &r.join("masteries.yaml")).await?;
    }

    step(tx, "Seeding factions\u{2026}").await;
    seed_factions(&db, &r.join("factions.yaml")).await?;

    step(tx, "Seeding races\u{2026}").await;
    seed_races(&db, &r.join("races.yaml")).await?;

    step(tx, "Seeding bloodlines\u{2026}").await;
    seed_bloodlines(&db, &r.join("bloodlines.yaml")).await?;

    step(tx, "Seeding universe regions\u{2026}").await;
    seed_regions(&db, &r.join("mapRegions.yaml")).await?;

    step(tx, "Seeding constellations\u{2026}").await;
    seed_constellations(&db, &r.join("mapConstellations.yaml")).await?;

    step(tx, "Seeding solar systems\u{2026}").await;
    seed_solar_systems(&db, &r.join("mapSolarSystems.yaml")).await?;

    step(tx, "Seeding stars\u{2026}").await;
    seed_stars(&db, &r.join("mapStars.yaml")).await?;

    step(tx, "Seeding planets\u{2026}").await;
    seed_planets(&db, &r.join("mapPlanets.yaml")).await?;

    step(tx, "Seeding stargates\u{2026}").await;
    seed_stargates(&db, &r.join("mapStargates.yaml")).await?;

    Ok(())
  }
  .await;

  db.enable_foreign_keys().await.map_err(|e| e.to_string())?;
  seed_result?;

  if let Some(build) = build_version {
    write_stored_sde_version(&build);
  }

  let _ = tokio::fs::remove_dir_all(&tmp).await;

  Ok(db)
}

async fn step(tx: &mut iced::futures::channel::mpsc::Sender<bootstrap::Message>, label: &str) {
  let _ = tx.send(bootstrap::Message::StepChanged(label.to_string())).await;
}

async fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
  let zip_path = zip_path.to_owned();
  let dest = dest.to_owned();
  tokio::task::spawn_blocking(move || {
    let file = std::fs::File::open(&zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
      let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
      let out_path = dest.join(entry.name());
      if entry.is_dir() {
        std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
      } else {
        if let Some(parent) = out_path.parent() {
          std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        out.write_all(&buf).map_err(|e| e.to_string())?;
      }
    }
    Ok::<(), String>(())
  })
  .await
  .map_err(|e| e.to_string())?
}

/// Locates the directory that directly contains `categories.yaml`, handling
/// ZIPs that wrap everything in a single top-level subdirectory.
async fn find_sde_root(extract_dir: &Path) -> PathBuf {
  if extract_dir.join("categories.yaml").exists() {
    return extract_dir.to_owned();
  }
  if let Ok(mut rd) = tokio::fs::read_dir(extract_dir).await {
    while let Ok(Some(entry)) = rd.next_entry().await {
      let path = entry.path();
      if path.is_dir() && path.join("categories.yaml").exists() {
        return path;
      }
    }
  }
  extract_dir.to_owned()
}

async fn read_sde_build_version(root: &Path) -> Option<String> {
  let data = tokio::fs::read_to_string(root.join("_sde.yaml")).await.ok()?;
  let map: HashMap<String, serde_yaml::Value> = serde_yaml::from_str(&data).ok()?;
  let sde = map.get("sde")?.as_mapping()?;
  let build = sde.get("buildNumber")?;
  Some(match build {
    serde_yaml::Value::String(s) => s.clone(),
    serde_yaml::Value::Number(n) => n.to_string(),
    other => serde_yaml::to_string(other).ok()?.trim().to_string(),
  })
}

fn sde_version_path() -> Option<PathBuf> {
  Some(dir_spec::state_home()?.join("pod").join("sde_version"))
}

fn read_stored_sde_version() -> Option<String> {
  let s = std::fs::read_to_string(sde_version_path()?).ok()?;
  Some(s.trim().to_string())
}

fn write_stored_sde_version(version: &str) {
  let Some(path) = sde_version_path() else {
    return;
  };
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).ok();
  }
  std::fs::write(path, version).ok();
}

#[tracing::instrument]
async fn read_yaml<T: serde::de::DeserializeOwned + Send + 'static>(path: &Path) -> Result<T, String> {
  let path = path.to_owned();
  tokio::task::spawn_blocking(move || {
    let data = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_yaml::from_slice::<T>(&data).map_err(|e| format!("parse {}: {e}", path.display()))
  })
  .await
  .map_err(|e| e.to_string())?
}

#[tracing::instrument(skip(db))]
async fn seed_categories(db: &pod_db::Repo, path: &Path) -> Result<(), String> {
  let entries: HashMap<i32, SdeCategoryEntry> = read_yaml(path).await?;

  let records: Vec<_> = entries
    .into_iter()
    .map(|(id, e)| {
      let mut m = models::ItemCategory::new(id, e.name.en());
      if !e.published {
        m.unpublish();
      }
      m
    })
    .collect();

  db.universe()
    .item_categories()
    .upsert_many(&records)
    .await
    .map_err(|e| e.to_string())
}

#[tracing::instrument(skip(db))]
async fn seed_groups(db: &pod_db::Repo, path: &Path) -> Result<(), String> {
  let entries: HashMap<i32, SdeGroupEntry> = read_yaml(path).await?;

  let records: Vec<_> = entries
    .into_iter()
    .map(|(id, e)| {
      let mut m = models::ItemGroup::new(id, e.category_id, e.name.en());
      if !e.published {
        m.unpublish();
      }
      m
    })
    .collect();

  db.universe()
    .item_groups()
    .upsert_many(&records)
    .await
    .map_err(|e| e.to_string())
}

#[tracing::instrument(skip(db))]
async fn seed_market_groups(db: &pod_db::Repo, path: &Path) -> Result<(), String> {
  let entries: HashMap<i32, SdeMarketGroupEntry> = read_yaml(path).await?;

  let records: Vec<_> = entries
    .into_iter()
    .map(|(id, e)| {
      let name = e.name.map(|n| n.en()).unwrap_or_else(|| format!("Market Group {id}"));
      let mut m = models::MarketGroup::new(id, name);
      m.set_parent_market_group_id(e.parent_group_id);
      m
    })
    .collect();

  db.universe()
    .market_groups()
    .upsert_many(&records)
    .await
    .map_err(|e| e.to_string())
}

#[tracing::instrument(skip(db))]
async fn seed_types(db: &pod_db::Repo, types_path: &Path, dogma_path: &Path) -> Result<(), String> {
  let entries: HashMap<i32, SdeTypeEntry> = read_yaml(types_path).await?;
  let dogma: HashMap<i32, SdeTypeDogmaEntry> = read_yaml(dogma_path).await?;

  let records: Vec<_> = entries
    .into_iter()
    .map(|(id, e)| {
      let d = dogma.get(&id);
      let dogma_attrs: Vec<models::DogmaAttributeEntry> = d
        .map(|d| {
          d.dogma_attributes
            .iter()
            .map(|a| models::DogmaAttributeEntry::new(a.attribute_id, a.value))
            .collect()
        })
        .unwrap_or_default();
      let dogma_effs: Vec<models::DogmaEffectEntry> = d
        .map(|d| {
          d.dogma_effects
            .iter()
            .map(|ef| models::DogmaEffectEntry::new(ef.effect_id, ef.is_default))
            .collect()
        })
        .unwrap_or_default();

      let mut m = models::ItemType::new(id, e.name.en());
      m.set_description(e.description.map(|d| d.en()).unwrap_or_default());
      m.set_item_group_id(e.group_id);
      m.set_market_group_id(e.market_group_id);
      m.set_mass(e.mass);
      m.set_volume(e.volume);
      m.set_capacity(e.capacity);
      m.set_portion_size(e.portion_size);
      m.set_radius(e.radius);
      m.set_icon_id(e.icon_id);
      m.set_graphic_id(e.graphic_id);
      m.set_published(e.published);
      *m.dogma_attributes_mut() = dogma_attrs;
      *m.dogma_effects_mut() = dogma_effs;
      m
    })
    .collect();

  db.universe()
    .item_types()
    .upsert_many(&records)
    .await
    .map_err(|e| e.to_string())
}

#[tracing::instrument(skip(db))]
async fn seed_factions(db: &pod_db::Repo, path: &Path) -> Result<(), String> {
  let entries: HashMap<i32, SdeFactionEntry> = read_yaml(path).await?;

  let records: Vec<_> = entries
    .into_iter()
    .map(|(id, e)| {
      let name = e.name.map(|n| n.en()).unwrap_or_else(|| format!("Faction {id}"));
      let mut m = models::Faction::new(id, name);
      m.set_size_factor(e.size_factor);
      m.set_solar_system_id(e.solar_system_id);
      m.set_is_unique(e.is_unique.unwrap_or(false));
      m
    })
    .collect();

  db.universe()
    .factions()
    .upsert_many(&records)
    .await
    .map_err(|e| e.to_string())
}

#[tracing::instrument(skip(db))]
async fn seed_races(db: &pod_db::Repo, path: &Path) -> Result<(), String> {
  let entries: HashMap<i32, SdeRaceEntry> = read_yaml(path).await?;

  let records: Vec<_> = entries
    .into_iter()
    .map(|(id, e)| {
      let name = e.name.map(|n| n.en()).unwrap_or_else(|| format!("Race {id}"));
      let mut m = models::Race::new(id, name);
      if let Some(aid) = e.alliance_id {
        m.set_alliance_id(aid);
      }
      m
    })
    .collect();

  db.universe()
    .races()
    .upsert_many(&records)
    .await
    .map_err(|e| e.to_string())
}

#[tracing::instrument(skip(db))]
async fn seed_bloodlines(db: &pod_db::Repo, path: &Path) -> Result<(), String> {
  let entries: HashMap<i32, SdeBloodlineEntry> = read_yaml(path).await?;

  let records: Vec<_> = entries
    .into_iter()
    .map(|(id, e)| {
      let name = e.name.map(|n| n.en()).unwrap_or_else(|| format!("Bloodline {id}"));
      let mut m = models::Bloodline::new(id, name);
      m.set_race_id(e.race_id);
      m.set_corporation_id(e.corporation_id);
      m.set_ship_item_type_id(e.ship_type_id);
      m.set_charisma(e.charisma);
      m.set_intelligence(e.intelligence);
      m.set_memory(e.memory);
      m.set_perception(e.perception);
      m.set_will_power(e.willpower);
      m
    })
    .collect();

  db.universe()
    .bloodlines()
    .upsert_many(&records)
    .await
    .map_err(|e| e.to_string())
}

#[tracing::instrument(skip(db))]
async fn seed_regions(db: &pod_db::Repo, path: &Path) -> Result<(), String> {
  let entries: HashMap<i32, SdeRegionMap> = read_yaml(path).await?;

  let records: Vec<_> = entries
    .into_iter()
    .map(|(id, e)| {
      let mut m = models::Region::new(id, format!("Region {id}"));
      m.set_description(e.description.map(|d| d.en()));
      m
    })
    .collect();

  db.universe()
    .regions()
    .upsert_many(&records)
    .await
    .map_err(|e| e.to_string())
}

#[tracing::instrument(skip(db))]
async fn seed_constellations(db: &pod_db::Repo, path: &Path) -> Result<(), String> {
  let entries: HashMap<i32, SdeConstellationMap> = read_yaml(path).await?;

  let records: Vec<_> = entries
    .into_iter()
    .map(|(id, e)| {
      let name = e.name.map(|n| n.en()).unwrap_or_else(|| format!("Constellation {id}"));
      let mut m = models::Constellation::new(id, &name);
      m.set_region_id(e.region_id);
      if let Some(pos) = e.center {
        m.set_position(pos.x, pos.y, pos.z);
      }
      m
    })
    .collect();

  db.universe()
    .constellations()
    .upsert_many(&records)
    .await
    .map_err(|e| e.to_string())
}

#[tracing::instrument(skip(db))]
async fn seed_solar_systems(db: &pod_db::Repo, path: &Path) -> Result<(), String> {
  let entries: HashMap<i32, SdeSolarSystemMap> = read_yaml(path).await?;

  let records: Vec<_> = entries
    .into_iter()
    .map(|(id, e)| {
      let name = e.name.map(|n| n.en()).unwrap_or_else(|| format!("System {id}"));
      let mut m = models::SolarSystem::new(id, &name);
      m.set_constellation_id(e.constellation_id);
      m.set_security_status(e.security_status.unwrap_or(0.0));
      m.set_security_class(e.security_class);
      m.set_star_id(e.star_id);
      if let Some(pos) = e.center {
        m.set_position(pos.x, pos.y, pos.z);
      }
      m
    })
    .collect();

  db.universe()
    .solar_systems()
    .upsert_many(&records)
    .await
    .map_err(|e| e.to_string())
}

#[tracing::instrument(skip(db))]
async fn seed_stars(db: &pod_db::Repo, path: &Path) -> Result<(), String> {
  let entries: HashMap<i32, SdeStarMap> = read_yaml(path).await?;

  let records: Vec<_> = entries
    .into_iter()
    .map(|(id, e)| {
      let mut m = models::Star::new(id, format!("Star {id}"));
      m.set_solar_system_id(e.solar_system_id);
      m.set_item_type_id(e.type_id);
      if let Some(r) = e.radius {
        m.set_radius(r as i64);
      }
      if let Some(stats) = e.statistics {
        m.set_spectral_class(stats.spectral_class.unwrap_or_default());
        m.set_age(stats.age.unwrap_or(0.0) as i64);
        m.set_luminosity(stats.luminosity.unwrap_or(0.0));
        m.set_temperature(stats.temperature.unwrap_or(0.0) as i32);
      }
      m
    })
    .collect();

  db.universe()
    .stars()
    .upsert_many(&records)
    .await
    .map_err(|e| e.to_string())
}

#[tracing::instrument(skip(db))]
async fn seed_planets(db: &pod_db::Repo, path: &Path) -> Result<(), String> {
  let entries: HashMap<i32, SdePlanetMap> = read_yaml(path).await?;

  let records: Vec<_> = entries
    .into_iter()
    .map(|(id, e)| {
      let mut m = models::Planet::new(id, format!("Planet {id}"));
      m.set_solar_system_id(e.solar_system_id);
      m.set_item_type_id(e.type_id);
      if let Some(pos) = e.position {
        m.set_position(pos.x, pos.y, pos.z);
      }
      m
    })
    .collect();

  db.universe()
    .planets()
    .upsert_many(&records)
    .await
    .map_err(|e| e.to_string())
}

#[tracing::instrument(skip(db))]
async fn seed_stargates(db: &pod_db::Repo, path: &Path) -> Result<(), String> {
  let entries: HashMap<i32, SdeStargateMap> = read_yaml(path).await?;

  let records: Vec<_> = entries
    .into_iter()
    .map(|(id, e)| {
      let mut m = models::Stargate::new(id, format!("Stargate {id}"));
      m.set_solar_system_id(e.solar_system_id);
      m.set_item_type_id(e.type_id);
      if let Some(pos) = e.position {
        m.set_position(pos.x, pos.y, pos.z);
      }
      if let Some(dest) = e.destination {
        m.set_destination(dest.stargate_id, dest.solar_system_id);
      }
      m
    })
    .collect();

  db.universe()
    .stargates()
    .upsert_many(&records)
    .await
    .map_err(|e| e.to_string())?;
  Ok(())
}

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
  #[serde(default = "default_true")]
  published: bool,
}

#[derive(Deserialize)]
struct SdeGroupEntry {
  #[serde(rename = "categoryID")]
  category_id: i32,
  name: LocalizedString,
  #[serde(default = "default_true")]
  published: bool,
}

#[derive(Deserialize)]
struct SdeMarketGroupEntry {
  name: Option<LocalizedString>,
  #[serde(rename = "parentGroupID")]
  parent_group_id: Option<i32>,
}

#[derive(Deserialize)]
struct SdeTypeEntry {
  name: LocalizedString,
  description: Option<LocalizedString>,
  #[serde(rename = "groupID")]
  group_id: i32,
  #[serde(rename = "marketGroupID")]
  market_group_id: Option<i32>,
  mass: Option<f64>,
  volume: Option<f64>,
  capacity: Option<f64>,
  #[serde(rename = "portionSize")]
  portion_size: Option<i32>,
  radius: Option<f64>,
  #[serde(default = "default_true")]
  published: bool,
  #[serde(rename = "iconID")]
  icon_id: Option<i32>,
  #[serde(rename = "graphicID")]
  graphic_id: Option<i32>,
}

#[derive(Default, Deserialize)]
struct SdeTypeDogmaEntry {
  #[serde(rename = "dogmaAttributes", default)]
  dogma_attributes: Vec<SdeDogmaAttribute>,
  #[serde(rename = "dogmaEffects", default)]
  dogma_effects: Vec<SdeDogmaEffect>,
}

#[derive(Deserialize)]
struct SdeDogmaAttribute {
  #[serde(rename = "attributeID")]
  attribute_id: i32,
  value: f64,
}

#[derive(Deserialize)]
struct SdeDogmaEffect {
  #[serde(rename = "effectID")]
  effect_id: i32,
  #[serde(rename = "isDefault", default)]
  is_default: bool,
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
struct SdeRegionMap {
  description: Option<LocalizedString>,
}

#[derive(Deserialize)]
struct SdeConstellationMap {
  name: Option<LocalizedString>,
  #[serde(rename = "regionID")]
  region_id: i32,
  center: Option<Position>,
}

#[derive(Deserialize)]
struct SdeSolarSystemMap {
  name: Option<LocalizedString>,
  #[serde(rename = "constellationID")]
  constellation_id: i32,
  #[serde(rename = "securityStatus")]
  security_status: Option<f64>,
  #[serde(rename = "securityClass")]
  security_class: Option<String>,
  #[serde(rename = "starID")]
  star_id: Option<i32>,
  center: Option<Position>,
}

#[derive(Deserialize)]
struct SdeStarMap {
  #[serde(rename = "solarSystemID")]
  solar_system_id: i32,
  #[serde(rename = "typeID")]
  type_id: i32,
  radius: Option<f64>,
  statistics: Option<SdeStarStats>,
}

#[derive(Deserialize)]
struct SdeStarStats {
  age: Option<f64>,
  luminosity: Option<f64>,
  #[serde(rename = "spectralClass")]
  spectral_class: Option<String>,
  temperature: Option<f64>,
}

#[derive(Deserialize)]
struct SdePlanetMap {
  #[serde(rename = "solarSystemID")]
  solar_system_id: i32,
  #[serde(rename = "typeID")]
  type_id: i32,
  position: Option<Position>,
}

#[derive(Deserialize)]
struct SdeStargateMap {
  #[serde(rename = "solarSystemID")]
  solar_system_id: i32,
  #[serde(rename = "typeID")]
  type_id: i32,
  destination: Option<SdeStargateDestination>,
  position: Option<Position>,
}

#[derive(Deserialize)]
struct SdeStargateDestination {
  #[serde(rename = "solarSystemID")]
  solar_system_id: i32,
  #[serde(rename = "stargateID")]
  stargate_id: i32,
}

#[derive(Clone, Default, Deserialize)]
struct Position {
  #[serde(default)]
  x: f64,
  #[serde(default)]
  y: f64,
  #[serde(default)]
  z: f64,
}

fn default_true() -> bool {
  true
}

fn default_one_f64() -> f64 {
  1.0
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

#[tracing::instrument(skip(db))]
async fn seed_certificates(db: &pod_db::Repo, path: &Path) -> Result<(), String> {
  let entries: HashMap<i32, SdeCertEntry> = read_yaml(path).await?;

  let records: Vec<models::Certificate> = entries
    .into_iter()
    .map(|(id, e)| {
      let grade = e.grade.unwrap_or(1).clamp(1, 5);
      models::Certificate {
        id,
        name: e.name.en(),
        description: e.description.map(|d| d.en()),
        grade: grade as u8,
        skills: e
          .skill_types
          .into_iter()
          .map(|(type_id, lvl)| {
            let clamp = |v: i32| v.clamp(0, 5) as u8;
            (
              type_id,
              [
                clamp(lvl.basic),
                clamp(lvl.improved),
                clamp(lvl.advanced),
                clamp(lvl.elite),
              ],
            )
          })
          .collect(),
      }
    })
    .collect();

  db.universe()
    .certificates()
    .upsert_many(&records)
    .await
    .map_err(|e| e.to_string())
}

#[tracing::instrument(skip(db))]
async fn seed_masteries(db: &pod_db::Repo, path: &Path) -> Result<(), String> {
  let raw_bytes = tokio::fs::read(path).await.map_err(|e| e.to_string())?;
  let raw: serde_yaml::Value =
    serde_yaml::from_slice(&raw_bytes).map_err(|e| format!("parse {}: {}", path.display(), e))?;

  let serde_yaml::Value::Mapping(outer) = raw else {
    return Ok(());
  };

  let mut mastery_entries: Vec<(i32, i32, Vec<i32>)> = Vec::new();
  for (ship_key, tiers_val) in outer {
    let ship_id = match &ship_key {
      serde_yaml::Value::Number(n) => n.as_i64().and_then(|n| i32::try_from(n).ok()),
      _ => None,
    };
    let (Some(ship_id), serde_yaml::Value::Mapping(tiers)) = (ship_id, tiers_val) else {
      continue;
    };
    for (tier_key, certs_val) in tiers {
      let tier_idx = match &tier_key {
        serde_yaml::Value::Number(n) => n.as_u64().and_then(|n| u8::try_from(n).ok()),
        serde_yaml::Value::String(s) => s.parse().ok(),
        _ => None,
      };
      let serde_yaml::Value::Sequence(certs) = certs_val else {
        continue;
      };
      let cert_ids: Vec<i32> = certs
        .into_iter()
        .filter_map(|v| match v {
          serde_yaml::Value::Number(n) => n.as_i64().and_then(|n| i32::try_from(n).ok()),
          _ => None,
        })
        .collect();
      if let Some(idx) = tier_idx.filter(|&t| t < 5)
        && !cert_ids.is_empty()
      {
        mastery_entries.push((ship_id, (idx as i32) + 1, cert_ids));
      }
    }
  }

  if !mastery_entries.is_empty() {
    db.universe()
      .certificates()
      .upsert_ship_masteries(&mastery_entries)
      .await
      .map_err(|e| e.to_string())?;
  }
  Ok(())
}
