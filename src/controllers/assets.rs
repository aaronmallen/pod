//! Assets controller: startup data fetch from the database.

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use pod_model::{Character, Corporation, Station};
use pod_ui::views::assets::{
  self, AssetRecord, AssetValuesData, CategoryValue, CharacterStructureCell, Message, State, StockpileItemStatus,
  StockpileWithStatus, TopItem,
};

use crate::services::{Services, character as character_service, corporation as corporation_service};

/// A uniform asset row abstracted over both the DB entity and the ESI response.
struct RawAssetRow {
  character_id: i64,
  is_blueprint_copy: Option<bool>,
  is_singleton: bool,
  item_id: i64,
  location_flag: String,
  location_id: i64,
  location_type: String,
  quantity: i32,
  type_id: i32,
}

/// Derived, pre-computed lookup tables needed to build `AssetRecord` values.
struct AssetMaps {
  /// category_key per category ID.
  cat_key_map: HashMap<i32, &'static str>,
  /// group name per group ID.
  group_name_map: HashMap<i32, String>,
  /// Set of item IDs that act as containers.
  is_container_set: HashSet<i64>,
  /// item_id → (location_id, location_type, location_flag, type_id).
  item_index: HashMap<i64, (i64, String, String, i32)>,
  /// Latest Jita price per type ID.
  price_cache: HashMap<i32, f64>,
  /// Station models keyed by NPC station ID.
  station_map: HashMap<i32, Station>,
  /// structure_id → (name, solar_system_id).
  structure_name_map: HashMap<i64, (String, i64)>,
  /// structure_id → name only (without solar_system_id), for path resolution.
  structure_name_only: HashMap<i64, String>,
  /// solar_system_id → name, only for systems referenced by structures.
  structure_system_name_map: HashMap<i32, String>,
  /// solar_system_id → name, for systems referenced by stations.
  system_name_map: HashMap<i32, String>,
  /// category ID per group ID.
  type_cat_map: HashMap<i32, i32>,
  /// group ID per type ID.
  type_group_map: HashMap<i32, i32>,
  /// display name per type ID.
  type_name_map: HashMap<i32, String>,
  /// packaged (or full) volume per type ID; 0.0 if unknown.
  type_volume_map: HashMap<i32, f64>,
}

fn icon_variant(is_blueprint_copy: Option<bool>) -> &'static str {
  match is_blueprint_copy {
    Some(true) => "bpc",
    Some(false) => "bpo",
    None => "icon",
  }
}

/// Creates a new assets state and kicks off a background asset load from the database.
pub fn new(
  characters: Vec<Character>,
  corporations: Vec<Corporation>,
  services: &Services,
) -> (State, iced::Task<Message>) {
  let chars_for_load = characters.clone();
  let char_ids: Vec<i64> = characters.iter().map(|c| *c.id()).collect();
  let state = assets::new(characters, corporations);
  let task = if let Some(db) = services.db.clone() {
    let esi = services.esi_client.clone();
    let db_icons = db.clone();
    let db_stockpiles = db.clone();
    let db_nav = db.clone();
    let nav_char_ids = char_ids.clone();
    let assets_task = iced::Task::perform(load_assets_from_db(db, chars_for_load, esi), Message::AssetsLoaded);
    let icons_task = iced::Task::perform(
      async move { load_all_cached_icons(db_icons).await },
      Message::ItemIconsLoaded,
    );
    let stockpiles_task = iced::Task::perform(load_stockpiles_with_status(db_stockpiles), Message::StockpilesLoaded);
    let nav_task = iced::Task::perform(nav_history(db_nav, nav_char_ids, 90), Message::NavHistoryLoaded);
    iced::Task::batch([assets_task, icons_task, stockpiles_task, nav_task])
  } else {
    iced::Task::none()
  };
  (state, task)
}

/// Creates a stockpile and refreshes the list.
pub async fn create_stockpile(
  db: pod_db::Repo,
  name: String,
  location_id: Option<i64>,
  character_id: Option<i64>,
  items: Vec<(i32, i32)>,
) -> Vec<StockpileWithStatus> {
  let _ = db
    .stockpiles()
    .create_stockpile(&name, location_id, character_id, &items)
    .await;
  load_stockpiles_with_status(db).await
}

/// Updates a stockpile and refreshes the list.
pub async fn update_stockpile(
  db: pod_db::Repo,
  id: i64,
  name: String,
  location_id: Option<i64>,
  character_id: Option<i64>,
  items: Vec<(i32, i32)>,
) -> Vec<StockpileWithStatus> {
  let _ = db
    .stockpiles()
    .update_stockpile(id, &name, location_id, character_id, &items)
    .await;
  load_stockpiles_with_status(db).await
}

/// Deletes a stockpile and refreshes the list.
pub async fn delete_stockpile(db: pod_db::Repo, id: i64) -> Vec<StockpileWithStatus> {
  let _ = db.stockpiles().delete_stockpile(id).await;
  load_stockpiles_with_status(db).await
}

/// Starts a background task to fetch and resolve corp hangar assets from ESI.
pub fn fetch_corp_assets(corp_id: i64, state: &State, services: &Services) -> iced::Task<Message> {
  let Some(corp) = state.corporations.iter().find(|c| *c.id() == corp_id).cloned() else {
    return iced::Task::done(Message::CorpAssetsLoaded(Vec::new()));
  };
  let (Some(db), Some(esi)) = (services.db.clone(), services.esi_client.clone()) else {
    return iced::Task::done(Message::CorpAssetsLoaded(Vec::new()));
  };
  let characters = state.characters.clone();
  iced::Task::perform(
    load_corp_assets_from_esi(corp, characters, db, esi),
    Message::CorpAssetsLoaded,
  )
}

fn category_name_to_key(name: &str) -> &'static str {
  match name {
    "Ship" => "ship",
    "Module" => "module",
    "Drone" => "drone",
    "Charge" => "charge",
    "Implant" | "Augmentation" => "implant",
    "Blueprint" => "blueprint",
    "Material" | "Mineral" => "material",
    "Skill" | "Skillbook" => "book",
    "Commodity" | "Ancient Relics" => "commodity",
    _ => "commodity",
  }
}

fn compute_depth(item_id: i64, item_index: &HashMap<i64, (i64, String, String, i32)>) -> usize {
  let mut depth = 0usize;
  let mut cursor_id = item_id;
  while let Some((loc_id, loc_type, _, _)) = item_index.get(&cursor_id) {
    if loc_type != "item" || depth > 20 {
      break;
    }
    if !item_index.contains_key(loc_id) {
      break;
    }
    depth += 1;
    cursor_id = *loc_id;
  }
  depth
}

/// Resolves a human-readable name for a non-item location terminus in a container path.
fn resolve_terminus_name(
  loc_id: &i64,
  loc_type: &str,
  station_map: &HashMap<i32, pod_model::Station>,
  system_name_map: &HashMap<i32, String>,
  structure_name_map: &HashMap<i64, String>,
) -> String {
  if loc_type == "station" && *loc_id < i32::MAX as i64 {
    station_map
      .get(&(*loc_id as i32))
      .map(|s| s.name().clone())
      .unwrap_or_else(|| format!("Station {}", loc_id))
  } else if loc_type == "solar_system" && *loc_id < i32::MAX as i64 {
    system_name_map
      .get(&(*loc_id as i32))
      .cloned()
      .unwrap_or_else(|| format!("System {}", loc_id))
  } else {
    structure_name_map
      .get(loc_id)
      .cloned()
      .unwrap_or_else(|| format!("Location {}", loc_id))
  }
}

/// Walks the item-index chain to find the nearest named container location.
///
/// `item_index` maps item_id → (location_id, location_type, location_flag, type_id).
fn resolve_container_path(
  parent_item_id: i64,
  item_index: &HashMap<i64, (i64, String, String, i32)>,
  type_name_map: &HashMap<i32, String>,
  station_map: &HashMap<i32, pod_model::Station>,
  system_name_map: &HashMap<i32, String>,
  structure_name_map: &HashMap<i64, String>,
) -> (String, i64) {
  let mut cursor_id = parent_item_id;
  let mut depth = 0usize;
  let mut last_container_id = parent_item_id;
  let mut last_flag = String::new();
  let mut last_type_id = 0i32;
  loop {
    let Some((loc_id, loc_type, loc_flag, type_id)) = item_index.get(&cursor_id) else {
      if cursor_id == parent_item_id {
        break;
      }
      let loc_name = structure_name_map
        .get(&cursor_id)
        .cloned()
        .unwrap_or_else(|| format!("Location {}", cursor_id));
      let flag = humanize_flag(&last_flag);
      let ctype = type_name_map
        .get(&last_type_id)
        .map(|n| n.as_str())
        .unwrap_or("Container");
      return (format!("{} · {} · {}", loc_name, flag, ctype), last_container_id);
    };
    if loc_type != "item" || depth > 20 {
      let loc_name = resolve_terminus_name(loc_id, loc_type, station_map, system_name_map, structure_name_map);
      let flag = humanize_flag(loc_flag);
      let ctype = type_name_map.get(type_id).map(|n| n.as_str()).unwrap_or("Container");
      return (format!("{} · {} · {}", loc_name, flag, ctype), cursor_id);
    }
    last_container_id = cursor_id;
    last_flag = loc_flag.clone();
    last_type_id = *type_id;
    cursor_id = *loc_id;
    depth += 1;
  }
  (String::new(), 0)
}

fn humanize_flag(flag: &str) -> &'static str {
  match flag {
    "Hangar" | "AssetSafety" => "Item Hangar",
    "CorpDeliveries" => "Corp Deliveries",
    "CorpSAG1" => "Corp Hangar 1",
    "CorpSAG2" => "Corp Hangar 2",
    "CorpSAG3" => "Corp Hangar 3",
    "CorpSAG4" => "Corp Hangar 4",
    "CorpSAG5" => "Corp Hangar 5",
    "CorpSAG6" => "Corp Hangar 6",
    "CorpSAG7" => "Corp Hangar 7",
    "ShipHangar" => "Ship Hangar",
    "FuelBay" => "Fuel Bay",
    _ => "Hangar",
  }
}

/// Resolves player structure names: DB cache first, ESI fallback for unknowns.
///
/// Returns a map of structure_id → (name, solar_system_id). The solar_system_id
/// is 0 when only a cached name is available (no ESI lookup was performed).
///
/// Any ESI successes are written back to the DB cache so future loads are instant.
#[tracing::instrument(skip_all)]
async fn resolve_structure_names(
  locs: &[(i64, i64)],
  characters: &[Character],
  esi: Option<&pod_esi::Client>,
  db: &pod_db::Repo,
) -> HashMap<i64, (String, i64)> {
  if locs.is_empty() {
    return HashMap::new();
  }

  let unique_struct_ids: Vec<i64> = unique_ids(locs.iter().map(|(id, _)| *id));

  let mut result: HashMap<i64, (String, i64)> = db
    .universe()
    .structure_cache()
    .find_by_ids(&unique_struct_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(id, name, sys_id)| (id, (name, sys_id.unwrap_or(0))))
    .collect();

  let missing: Vec<i64> = unique_struct_ids
    .into_iter()
    .filter(|id| !result.contains_key(id))
    .collect();
  if missing.is_empty() || esi.is_none() {
    return result;
  }

  let newly_resolved = esi_resolve_missing_structures(&missing, locs, characters, esi.unwrap(), db, &mut result).await;

  if !newly_resolved.is_empty() {
    let _ = db.universe().structure_cache().upsert_many(&newly_resolved).await;
  }

  result
}

/// Queries ESI for each missing structure, trying available characters in turn.
async fn esi_resolve_missing_structures(
  missing: &[i64],
  locs: &[(i64, i64)],
  characters: &[Character],
  esi: &pod_esi::Client,
  db: &pod_db::Repo,
  result: &mut HashMap<i64, (String, i64)>,
) -> Vec<(i64, String, Option<i64>)> {
  let char_map: HashMap<i64, &Character> = characters.iter().map(|c| (*c.id(), c)).collect();
  let struct_chars = build_struct_chars(missing, locs);
  let mut newly_resolved = Vec::new();
  for (struct_id, char_ids) in &struct_chars {
    resolve_single_structure(*struct_id, char_ids, &char_map, esi, db, result, &mut newly_resolved).await;
  }
  newly_resolved
}

/// Builds a map of structure_id → char_ids that can see it, for the missing subset.
fn build_struct_chars(missing: &[i64], locs: &[(i64, i64)]) -> HashMap<i64, Vec<i64>> {
  let mut struct_chars: HashMap<i64, Vec<i64>> = HashMap::new();
  for &(loc_id, char_id) in locs {
    if missing.contains(&loc_id) {
      struct_chars.entry(loc_id).or_default().push(char_id);
    }
  }
  struct_chars
}

/// Attempts to resolve one structure via ESI, trying each candidate character.
async fn resolve_single_structure(
  struct_id: i64,
  char_ids: &[i64],
  char_map: &HashMap<i64, &Character>,
  esi: &pod_esi::Client,
  db: &pod_db::Repo,
  result: &mut HashMap<i64, (String, i64)>,
  newly_resolved: &mut Vec<(i64, String, Option<i64>)>,
) {
  for &char_id in char_ids {
    let Some(character) = char_map.get(&char_id) else {
      continue;
    };
    let Some(token) = character_service::ensure_valid_token(character, esi, db).await else {
      tracing::warn!("assets: token unavailable for character {char_id}, skipping structure {struct_id}");
      continue;
    };
    let grant = character_service::refresh_grant(character, &token);
    match esi.universe().structure(struct_id).auth(&grant).detail().await {
      Ok(info) => {
        newly_resolved.push((struct_id, info.name.clone(), Some(info.solar_system_id)));
        result.insert(struct_id, (info.name, info.solar_system_id));
        return;
      }
      Err(e) => {
        tracing::warn!("assets: ESI structure {struct_id} failed for character {char_id}: {e}");
      }
    }
  }
  tracing::warn!("assets: could not resolve structure {struct_id} — will show as Location ID");
}

/// Returns unique IDs collected from an iterator.
fn unique_ids<T: Eq + std::hash::Hash>(iter: impl Iterator<Item = T>) -> Vec<T> {
  iter.collect::<HashSet<_>>().into_iter().collect()
}

/// Fetches type, group, and category lookup maps from the DB and derives simpler maps.
async fn load_type_maps(
  db: &pod_db::Repo,
  type_ids: &[i32],
) -> (
  HashMap<i32, String>,
  HashMap<i32, f64>,
  HashMap<i32, i32>,
  HashMap<i32, String>,
  HashMap<i32, i32>,
  HashMap<i32, &'static str>,
) {
  let type_rows = db
    .universe()
    .item_types()
    .find_by_ids(type_ids)
    .await
    .unwrap_or_default();
  let type_name_map: HashMap<i32, String> = type_rows.iter().map(|r| (r.id, r.name.clone())).collect();
  let type_volume_map: HashMap<i32, f64> = type_rows
    .iter()
    .map(|r| (r.id, r.packaged_volume.or(r.volume).unwrap_or(0.0)))
    .collect();
  let type_group_map: HashMap<i32, i32> = type_rows.iter().map(|r| (r.id, r.item_group_id)).collect();

  let group_ids: Vec<i32> = unique_ids(type_rows.iter().map(|r| r.item_group_id));
  let group_rows = db
    .universe()
    .item_groups()
    .find_by_ids(&group_ids)
    .await
    .unwrap_or_default();
  let group_name_map: HashMap<i32, String> = group_rows.iter().map(|r| (r.id, r.name.clone())).collect();
  let type_cat_map: HashMap<i32, i32> = group_rows.iter().map(|r| (r.id, r.item_category_id)).collect();

  let cat_ids: Vec<i32> = unique_ids(group_rows.iter().map(|r| r.item_category_id));
  let cat_rows = db
    .universe()
    .item_categories()
    .find_by_ids(&cat_ids)
    .await
    .unwrap_or_default();
  let cat_key_map: HashMap<i32, &'static str> =
    cat_rows.iter().map(|r| (r.id, category_name_to_key(&r.name))).collect();

  (
    type_name_map,
    type_volume_map,
    type_group_map,
    group_name_map,
    type_cat_map,
    cat_key_map,
  )
}

/// Loads station records from the DB for the given station IDs.
async fn load_station_map(db: &pod_db::Repo, station_ids: &[i32]) -> HashMap<i32, Station> {
  db.universe()
    .stations()
    .find_by_ids(station_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|s| (*s.id(), s))
    .collect()
}

/// Fetches any station IDs missing from the cache via ESI and persists them.
async fn fetch_missing_stations(
  db: &pod_db::Repo,
  esi: &pod_esi::Client,
  station_ids: &[i32],
  station_map: &mut HashMap<i32, Station>,
) {
  let missing: Vec<i32> = station_ids
    .iter()
    .copied()
    .filter(|id| !station_map.contains_key(id))
    .collect();
  for id in missing {
    if let Ok(esi_station) = esi.universe().station(id as i64).await {
      let station = Station::from(esi_station);
      let _ = db.universe().stations().upsert(&station).await;
      station_map.insert(id, station);
    }
  }
}

/// Builds a solar-system name map from systems referenced by the station map.
async fn load_system_name_map(db: &pod_db::Repo, station_map: &HashMap<i32, Station>) -> HashMap<i32, String> {
  let sys_ids: Vec<i32> = unique_ids(station_map.values().map(|s| *s.solar_system_id()));
  db.universe()
    .solar_systems()
    .find_by_ids(&sys_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.id, r.name))
    .collect()
}

/// Builds the item index: item_id → (location_id, location_type, location_flag, type_id).
fn build_item_index(rows: &[RawAssetRow]) -> HashMap<i64, (i64, String, String, i32)> {
  rows
    .iter()
    .map(|a| {
      (
        a.item_id,
        (
          a.location_id,
          a.location_type.clone(),
          a.location_flag.clone(),
          a.type_id,
        ),
      )
    })
    .collect()
}

/// Returns the set of item IDs that are containers (i.e. other items are located inside them).
fn build_is_container_set(item_index: &HashMap<i64, (i64, String, String, i32)>) -> HashSet<i64> {
  let mut set = HashSet::new();
  for (loc_id, loc_type, _, _) in item_index.values() {
    if loc_type == "item" {
      set.insert(*loc_id);
    }
  }
  set
}

/// Resolves solar-system names for structures referenced in `structure_name_map`.
async fn load_structure_sys_name_map(
  db: &pod_db::Repo,
  structure_name_map: &HashMap<i64, (String, i64)>,
) -> HashMap<i32, String> {
  let solar_sys_ids: Vec<i32> =
    unique_ids(structure_name_map.values().filter_map(
      |(_, sys_id)| {
        if *sys_id > 0 { i32::try_from(*sys_id).ok() } else { None }
      },
    ));
  if solar_sys_ids.is_empty() {
    return HashMap::new();
  }
  db.universe()
    .solar_systems()
    .find_by_ids(&solar_sys_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|s| (s.id, s.name))
    .collect()
}

/// Builds the price cache from the latest prices for all known type IDs.
async fn build_price_cache(db: &pod_db::Repo, type_ids: &[i32]) -> HashMap<i32, f64> {
  db.prices().latest_prices(type_ids).await.unwrap_or_default()
}

/// Extracts the unique set of station-type location IDs from a slice of asset rows.
fn station_location_ids(rows: &[RawAssetRow]) -> Vec<i32> {
  unique_ids(
    rows
      .iter()
      .filter(|a| a.location_type == "station" && a.location_id < i32::MAX as i64)
      .map(|a| a.location_id as i32),
  )
}

/// Resolves the top-level location name and solar-system name for one asset row.
fn resolve_location(row: &RawAssetRow, maps: &AssetMaps) -> (String, String) {
  let is_at_structure = row.location_id >= i32::MAX as i64 && !maps.item_index.contains_key(&row.location_id);
  if row.location_type == "station" && row.location_id < i32::MAX as i64 {
    resolve_station_location(row.location_id, &maps.station_map, &maps.system_name_map)
  } else if (row.location_type == "solar_system" || row.location_type == "space") && row.location_id < i32::MAX as i64 {
    resolve_solar_system_location(row.location_id, &maps.system_name_map)
  } else if is_at_structure {
    resolve_structure_location(
      row.location_id,
      &maps.structure_name_map,
      &maps.structure_system_name_map,
    )
  } else {
    (String::new(), String::new())
  }
}

/// Resolves (location_name, system_name) for a station location.
fn resolve_station_location(
  location_id: i64,
  station_map: &HashMap<i32, Station>,
  system_name_map: &HashMap<i32, String>,
) -> (String, String) {
  let station = station_map.get(&(location_id as i32));
  let loc = station
    .map(|s| s.name().clone())
    .unwrap_or_else(|| format!("Station {}", location_id));
  let sys = station
    .and_then(|s| system_name_map.get(s.solar_system_id()))
    .cloned()
    .unwrap_or_default();
  (loc, sys)
}

/// Resolves (location_name, system_name) for a solar-system location.
fn resolve_solar_system_location(location_id: i64, system_name_map: &HashMap<i32, String>) -> (String, String) {
  let sys = system_name_map
    .get(&(location_id as i32))
    .cloned()
    .unwrap_or_else(|| format!("System {}", location_id));
  (sys.clone(), sys)
}

/// Resolves (location_name, system_name) for a player structure location.
fn resolve_structure_location(
  location_id: i64,
  structure_name_map: &HashMap<i64, (String, i64)>,
  structure_system_name_map: &HashMap<i32, String>,
) -> (String, String) {
  let (name, solar_sys_id) = structure_name_map
    .get(&location_id)
    .map(|(n, s)| (n.clone(), *s))
    .unwrap_or_else(|| (format!("Location {}", location_id), 0));
  let sys = if solar_sys_id > 0 {
    i32::try_from(solar_sys_id)
      .ok()
      .and_then(|id| structure_system_name_map.get(&id).cloned())
      .unwrap_or_default()
  } else {
    String::new()
  };
  (name, sys)
}

/// Resolves the container path and container ID for one asset row.
fn resolve_container_for_row(row: &RawAssetRow, maps: &AssetMaps) -> (String, i64) {
  let is_at_structure = row.location_id >= i32::MAX as i64 && !maps.item_index.contains_key(&row.location_id);
  if row.location_type != "item" || is_at_structure {
    return (String::new(), 0);
  }
  resolve_container_path(
    row.location_id,
    &maps.item_index,
    &maps.type_name_map,
    &maps.station_map,
    &maps.system_name_map,
    &maps.structure_name_only,
  )
}

/// Maps one `RawAssetRow` to an `AssetRecord` using the resolved lookup maps.
fn map_row_to_record(row: RawAssetRow, owner_id: i64, maps: &AssetMaps) -> AssetRecord {
  let type_name = maps
    .type_name_map
    .get(&row.type_id)
    .cloned()
    .unwrap_or_else(|| format!("Type {}", row.type_id));
  let group_id = maps.type_group_map.get(&row.type_id).copied();
  let group_name = group_id
    .and_then(|g| maps.group_name_map.get(&g))
    .cloned()
    .unwrap_or_default();
  let cat_key = group_id
    .and_then(|g| maps.type_cat_map.get(&g).copied())
    .and_then(|c| maps.cat_key_map.get(&c).copied())
    .unwrap_or("commodity");
  let volume = maps.type_volume_map.get(&row.type_id).copied().unwrap_or(0.0);
  let unit_price = if row.is_blueprint_copy == Some(true) {
    0.0
  } else {
    maps.price_cache.get(&row.type_id).copied().unwrap_or(0.0)
  };
  let (location_name, system_name) = resolve_location(&row, maps);
  let (container_path, container_id) = resolve_container_for_row(&row, maps);
  let depth = compute_depth(row.item_id, &maps.item_index);
  let is_container = maps.is_container_set.contains(&row.item_id);
  AssetRecord {
    item_id: row.item_id,
    character_id: owner_id,
    type_id: row.type_id,
    type_name,
    group_name,
    category_key: cat_key.to_string(),
    unit_price,
    volume,
    quantity: row.quantity as i64,
    location_id: row.location_id,
    location_name,
    system_name,
    is_singleton: row.is_singleton,
    container_path,
    container_id,
    depth,
    icon_variant: icon_variant(row.is_blueprint_copy).to_string(),
    is_container,
  }
}

/// Builds `AssetMaps` from all resolved DB / ESI data.
#[allow(clippy::too_many_arguments)]
async fn build_asset_maps(
  db: &pod_db::Repo,
  type_ids: &[i32],
  structure_locs: Vec<(i64, i64)>,
  characters: &[Character],
  esi: Option<&pod_esi::Client>,
  station_map: HashMap<i32, Station>,
  item_index: HashMap<i64, (i64, String, String, i32)>,
  is_container_set: HashSet<i64>,
) -> AssetMaps {
  let (
    (type_name_map, type_volume_map, type_group_map, group_name_map, type_cat_map, cat_key_map),
    system_name_map,
    structure_name_map,
    price_cache,
  ) = tokio::join!(
    load_type_maps(db, type_ids),
    load_system_name_map(db, &station_map),
    resolve_structure_names(&structure_locs, characters, esi, db),
    build_price_cache(db, type_ids),
  );
  let structure_system_name_map = load_structure_sys_name_map(db, &structure_name_map).await;
  let structure_name_only: HashMap<i64, String> = structure_name_map
    .iter()
    .map(|(&id, (name, _))| (id, name.clone()))
    .collect();
  AssetMaps {
    cat_key_map,
    group_name_map,
    is_container_set,
    item_index,
    price_cache,
    station_map,
    structure_name_map,
    structure_name_only,
    structure_system_name_map,
    system_name_map,
    type_cat_map,
    type_group_map,
    type_name_map,
    type_volume_map,
  }
}

async fn load_assets_from_db(
  db: pod_db::Repo,
  characters: Vec<Character>,
  esi: Option<pod_esi::Client>,
) -> Vec<AssetRecord> {
  if characters.is_empty() {
    return Vec::new();
  }
  let char_ids: Vec<i64> = characters.iter().map(|c| *c.id()).collect();
  let asset_rows = db
    .characters()
    .assets_for_character_ids(&char_ids)
    .await
    .unwrap_or_default();
  if asset_rows.is_empty() {
    return Vec::new();
  }
  let rows: Vec<RawAssetRow> = asset_rows
    .into_iter()
    .map(|a| RawAssetRow {
      character_id: a.character_id,
      item_id: a.item_id,
      is_blueprint_copy: a.is_blueprint_copy,
      is_singleton: a.is_singleton,
      location_flag: a.location_flag,
      location_id: a.location_id,
      location_type: a.location_type,
      quantity: a.quantity,
      type_id: a.type_id,
    })
    .collect();
  let type_ids = unique_ids(rows.iter().map(|a| a.type_id));
  let station_ids = station_location_ids(&rows);
  let mut station_map = load_station_map(&db, &station_ids).await;
  if let Some(ref esi_client) = esi {
    fetch_missing_stations(&db, esi_client, &station_ids, &mut station_map).await;
  }
  let item_index = build_item_index(&rows);
  let is_container_set = build_is_container_set(&item_index);
  let structure_locs: Vec<(i64, i64)> = rows
    .iter()
    .filter(|a| a.location_id >= i32::MAX as i64 && !item_index.contains_key(&a.location_id))
    .map(|a| (a.location_id, a.character_id))
    .collect();
  let maps = build_asset_maps(
    &db,
    &type_ids,
    structure_locs,
    &characters,
    esi.as_ref(),
    station_map,
    item_index,
    is_container_set,
  )
  .await;
  rows
    .into_iter()
    .map(|a| {
      let char_id = a.character_id;
      map_row_to_record(a, char_id, &maps)
    })
    .collect()
}

async fn load_corp_assets_from_esi(
  corp: Corporation,
  characters: Vec<Character>,
  db: pod_db::Repo,
  esi: pod_esi::Client,
) -> Vec<AssetRecord> {
  let Some(token) = corporation_service::ensure_valid_token(&corp, &esi, &db).await else {
    return Vec::new();
  };
  let grant = corporation_service::refresh_grant(&corp, &token);
  let corp_id = *corp.id();
  let asset_rows = match esi.corporation(corp_id).auth(&grant).assets().await {
    Ok(rows) => rows,
    Err(_) => return Vec::new(),
  };
  if asset_rows.is_empty() {
    return Vec::new();
  }
  let rows: Vec<RawAssetRow> = asset_rows
    .into_iter()
    .map(|a| RawAssetRow {
      character_id: corp_id,
      item_id: a.item_id,
      is_blueprint_copy: a.is_blueprint_copy,
      is_singleton: a.is_singleton,
      location_flag: a.location_flag,
      location_id: a.location_id,
      location_type: a.location_type,
      quantity: a.quantity,
      type_id: a.type_id,
    })
    .collect();
  let type_ids = unique_ids(rows.iter().map(|a| a.type_id));
  let station_ids = station_location_ids(&rows);
  let mut station_map = load_station_map(&db, &station_ids).await;
  fetch_missing_stations(&db, &esi, &station_ids, &mut station_map).await;
  let item_index = build_item_index(&rows);
  let is_container_set = build_is_container_set(&item_index);
  let corp_char_id = characters.first().map(|c| *c.id()).unwrap_or(corp_id);
  let structure_locs: Vec<(i64, i64)> = rows
    .iter()
    .filter(|a| a.location_id >= i32::MAX as i64 && !item_index.contains_key(&a.location_id))
    .map(|a| (a.location_id, corp_char_id))
    .collect();
  let maps = build_asset_maps(
    &db,
    &type_ids,
    structure_locs,
    &characters,
    Some(&esi),
    station_map,
    item_index,
    is_container_set,
  )
  .await;
  rows.into_iter().map(|a| map_row_to_record(a, corp_id, &maps)).collect()
}

async fn load_all_cached_icons(db: pod_db::Repo) -> Vec<(i32, String, Vec<u8>)> {
  db.universe().type_icons().find_all().await.unwrap_or_default()
}

async fn load_stockpiles_with_status(db: pod_db::Repo) -> Vec<StockpileWithStatus> {
  let piles = match db.stockpiles().list_stockpiles().await {
    Ok(p) => p,
    Err(_) => return Vec::new(),
  };
  let mut result = Vec::with_capacity(piles.len());
  for pile in piles {
    let statuses = db.stockpiles().stockpile_fill_status(pile.id).await.unwrap_or_default();
    let total_target: i64 = statuses.iter().map(|s| s.target_quantity as i64).sum();
    let total_have: i64 = statuses
      .iter()
      .map(|s| (s.have_quantity).min(s.target_quantity as i64))
      .sum();
    let overall_pct = if total_target == 0 {
      1.0_f32
    } else {
      (total_have as f32 / total_target as f32).clamp(0.0, 1.0)
    };
    let ready = statuses.iter().all(|s| s.have_quantity >= s.target_quantity as i64);
    let items = statuses.into_iter().map(stockpile_item_status).collect();
    result.push(StockpileWithStatus {
      id: pile.id,
      name: pile.name,
      location_id: pile.location_id,
      character_id: pile.character_id,
      items,
      overall_pct,
      ready,
    });
  }
  result
}

/// Converts a raw fill-status row into the UI `StockpileItemStatus` type.
fn stockpile_item_status(s: pod_db::StockpileItemStatus) -> StockpileItemStatus {
  let pct = if s.target_quantity == 0 {
    1.0_f32
  } else {
    (s.have_quantity as f32 / s.target_quantity as f32).clamp(0.0, 1.0)
  };
  StockpileItemStatus {
    type_id: s.type_id,
    target_quantity: s.target_quantity,
    have_quantity: s.have_quantity,
    type_name: s.type_name,
    pct,
  }
}

pub async fn nav_history(db: pod_db::Repo, char_ids: Vec<i64>, days: u32) -> Vec<(NaiveDate, f64)> {
  db.prices().nav_history(&char_ids, days).await.unwrap_or_default()
}

/// Computes the full asset values breakdown for the Values tab.
pub async fn asset_values_breakdown(
  assets: Vec<AssetRecord>,
  characters: Vec<pod_model::Character>,
) -> AssetValuesData {
  let char_name_map: HashMap<i64, String> = characters.iter().map(|c| (*c.id(), c.name().clone())).collect();
  let valued: Vec<(&AssetRecord, f64)> = assets
    .iter()
    .map(|a| {
      let value = a.unit_price * a.quantity as f64;
      (a, value)
    })
    .collect();
  let total_value: f64 = valued.iter().map(|(_, v)| v).sum();
  let character_structure_cells = build_char_structure_cells(&valued, &char_name_map);
  let category_breakdown = build_category_breakdown(&valued, total_value);
  let top_items = build_top_items(&valued);
  AssetValuesData {
    character_structure_cells,
    category_breakdown,
    top_items,
    total_value,
  }
}

/// Builds the per-character, per-structure value matrix for the Values tab.
fn build_char_structure_cells(
  valued: &[(&AssetRecord, f64)],
  char_name_map: &HashMap<i64, String>,
) -> Vec<CharacterStructureCell> {
  let mut matrix: HashMap<(i64, String), (String, String, f64)> = HashMap::new();
  for (a, value) in valued {
    let struct_name = if a.container_path.is_empty() {
      a.location_name.clone()
    } else {
      a.container_path
        .split(" · ")
        .next()
        .unwrap_or(&a.location_name)
        .to_string()
    };
    let entry = matrix
      .entry((a.character_id, struct_name.clone()))
      .or_insert_with(|| (struct_name.clone(), struct_name, 0.0));
    entry.2 += value;
  }
  let mut cells: Vec<CharacterStructureCell> = matrix
    .into_iter()
    .map(
      |((char_id, struct_id), (_sid, struct_name, value))| CharacterStructureCell {
        character_id: char_id,
        character_name: char_name_map
          .get(&char_id)
          .cloned()
          .unwrap_or_else(|| char_id.to_string()),
        structure_id: struct_id,
        structure_name: struct_name,
        value,
      },
    )
    .collect();
  cells.sort_by(|a, b| {
    a.character_id
      .cmp(&b.character_id)
      .then_with(|| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal))
  });
  cells
}

/// Builds the category breakdown sorted by value descending.
fn build_category_breakdown(valued: &[(&AssetRecord, f64)], total_value: f64) -> Vec<CategoryValue> {
  let mut cat_map: HashMap<String, f64> = HashMap::new();
  for (a, value) in valued {
    let cat = if a.category_key == "all" || a.category_key.is_empty() {
      "commodity"
    } else {
      &a.category_key
    };
    *cat_map.entry(cat.to_string()).or_insert(0.0) += value;
  }
  let mut breakdown: Vec<CategoryValue> = cat_map
    .into_iter()
    .filter(|(_, v)| *v > 0.0)
    .map(|(cat_key, value)| {
      let pct = if total_value > 0.0 { value / total_value } else { 0.0 };
      CategoryValue {
        category_name: cat_key,
        value,
        pct,
      }
    })
    .collect();
  breakdown.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
  breakdown
}

/// Builds the top-10 most valuable items list.
fn build_top_items(valued: &[(&AssetRecord, f64)]) -> Vec<TopItem> {
  let mut top_map: HashMap<i32, (String, String, i64, f64)> = HashMap::new();
  for (a, value) in valued {
    let entry = top_map
      .entry(a.type_id)
      .or_insert_with(|| (a.type_name.clone(), a.category_key.clone(), 0, 0.0));
    entry.2 += a.quantity;
    entry.3 += value;
  }
  let mut items: Vec<TopItem> = top_map
    .into_iter()
    .map(|(type_id, (type_name, category_name, total_quantity, value))| TopItem {
      type_id,
      type_name,
      category_name,
      total_quantity,
      value,
    })
    .collect();
  items.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
  items.truncate(10);
  items
}

pub async fn fetch_type_icons(
  items: Vec<(i32, String)>,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Vec<(i32, String, Vec<u8>)> {
  let mut by_variant: HashMap<String, Vec<i32>> = HashMap::new();
  for (type_id, variant) in &items {
    by_variant.entry(variant.clone()).or_default().push(*type_id);
  }
  let mut cached: Vec<(i32, String, Vec<u8>)> = Vec::new();
  for (variant, ids) in &by_variant {
    let rows = db
      .universe()
      .type_icons()
      .find_by_ids(ids, variant)
      .await
      .unwrap_or_default();
    cached.extend(rows.into_iter().map(|(id, data)| (id, variant.clone(), data)));
  }
  let cached_keys: HashSet<(i32, String)> = cached.iter().map(|(id, v, _)| (*id, v.clone())).collect();
  let mut results = cached;
  for (type_id, variant) in items {
    if cached_keys.contains(&(type_id, variant.clone())) {
      continue;
    }
    let fetch_result = match variant.as_str() {
      "bpc" => esi.images().type_bpc(type_id as i64, 32).await,
      "bpo" => esi.images().type_bpo(type_id as i64, 32).await,
      _ => esi.images().type_icon(type_id as i64, 32).await,
    };
    if let Ok(bytes) = fetch_result {
      let _ = db
        .universe()
        .type_icons()
        .upsert(type_id, &variant, bytes.clone())
        .await;
      results.push((type_id, variant, bytes));
    }
  }
  results
}

#[cfg(test)]
mod tests {
  use super::*;

  mod category_name_to_key {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_known_categories() {
      assert_eq!(category_name_to_key("Ship"), "ship");
      assert_eq!(category_name_to_key("Module"), "module");
      assert_eq!(category_name_to_key("Drone"), "drone");
      assert_eq!(category_name_to_key("Charge"), "charge");
      assert_eq!(category_name_to_key("Implant"), "implant");
      assert_eq!(category_name_to_key("Augmentation"), "implant");
      assert_eq!(category_name_to_key("Blueprint"), "blueprint");
      assert_eq!(category_name_to_key("Material"), "material");
      assert_eq!(category_name_to_key("Mineral"), "material");
      assert_eq!(category_name_to_key("Skill"), "book");
      assert_eq!(category_name_to_key("Skillbook"), "book");
      assert_eq!(category_name_to_key("Commodity"), "commodity");
      assert_eq!(category_name_to_key("Ancient Relics"), "commodity");
    }

    #[test]
    fn it_falls_back_to_commodity_for_unknown() {
      assert_eq!(category_name_to_key("Unknown"), "commodity");
      assert_eq!(category_name_to_key(""), "commodity");
    }
  }

  mod humanize_flag {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_known_flags() {
      assert_eq!(humanize_flag("Hangar"), "Item Hangar");
      assert_eq!(humanize_flag("AssetSafety"), "Item Hangar");
      assert_eq!(humanize_flag("CorpDeliveries"), "Corp Deliveries");
      assert_eq!(humanize_flag("CorpSAG1"), "Corp Hangar 1");
      assert_eq!(humanize_flag("CorpSAG7"), "Corp Hangar 7");
      assert_eq!(humanize_flag("ShipHangar"), "Ship Hangar");
      assert_eq!(humanize_flag("FuelBay"), "Fuel Bay");
    }

    #[test]
    fn it_falls_back_to_hangar_for_unknown() {
      assert_eq!(humanize_flag("Cargo"), "Hangar");
      assert_eq!(humanize_flag(""), "Hangar");
    }
  }

  mod icon_variant {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_bpc_for_copy() {
      assert_eq!(icon_variant(Some(true)), "bpc");
    }

    #[test]
    fn it_returns_bpo_for_original() {
      assert_eq!(icon_variant(Some(false)), "bpo");
    }

    #[test]
    fn it_returns_icon_for_none() {
      assert_eq!(icon_variant(None), "icon");
    }
  }

  mod unique_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_deduplicates() {
      let mut result = unique_ids([1i32, 2, 2, 3, 1].iter().copied());
      result.sort_unstable();

      assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn it_handles_empty() {
      let result = unique_ids(std::iter::empty::<i32>());

      assert_eq!(result, Vec::<i32>::new());
    }
  }

  mod build_item_index {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_row(item_id: i64, location_id: i64, location_type: &str, flag: &str, type_id: i32) -> RawAssetRow {
      RawAssetRow {
        character_id: 1,
        is_blueprint_copy: None,
        is_singleton: false,
        item_id,
        location_flag: flag.to_string(),
        location_id,
        location_type: location_type.to_string(),
        quantity: 1,
        type_id,
      }
    }

    #[test]
    fn it_indexes_rows_by_item_id() {
      let rows = vec![
        make_row(10, 100, "station", "Hangar", 42),
        make_row(20, 10, "item", "Cargo", 99),
      ];

      let index = build_item_index(&rows);

      assert_eq!(
        index.get(&10),
        Some(&(100, "station".to_string(), "Hangar".to_string(), 42))
      );
      assert_eq!(index.get(&20), Some(&(10, "item".to_string(), "Cargo".to_string(), 99)));
    }
  }

  mod build_is_container_set {
    use super::*;

    #[test]
    fn it_marks_items_that_contain_other_items() {
      let mut index: HashMap<i64, (i64, String, String, i32)> = HashMap::new();
      index.insert(10, (100, "station".to_string(), "Hangar".to_string(), 1));
      index.insert(20, (10, "item".to_string(), "Cargo".to_string(), 2));
      index.insert(30, (10, "item".to_string(), "Cargo".to_string(), 3));

      let containers = build_is_container_set(&index);

      assert!(containers.contains(&10));
      assert!(!containers.contains(&20));
      assert!(!containers.contains(&30));
      assert!(!containers.contains(&100));
    }
  }

  mod compute_depth {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_zero_for_top_level_items() {
      let index: HashMap<i64, (i64, String, String, i32)> = HashMap::new();

      assert_eq!(compute_depth(999, &index), 0);
    }

    #[test]
    fn it_counts_nesting_levels() {
      let mut index: HashMap<i64, (i64, String, String, i32)> = HashMap::new();
      index.insert(10, (100, "station".to_string(), "Hangar".to_string(), 1));
      index.insert(20, (10, "item".to_string(), "Cargo".to_string(), 2));
      index.insert(30, (20, "item".to_string(), "Cargo".to_string(), 3));

      assert_eq!(compute_depth(10, &index), 0);
      assert_eq!(compute_depth(20, &index), 1);
      assert_eq!(compute_depth(30, &index), 2);
    }
  }

  mod build_struct_chars {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_groups_char_ids_by_structure() {
      let missing = vec![111i64, 222];
      let locs = vec![(111i64, 1i64), (111, 2), (222, 3), (999, 4)];

      let mut result = build_struct_chars(&missing, &locs);
      for v in result.values_mut() {
        v.sort_unstable();
      }

      assert_eq!(result[&111], vec![1, 2]);
      assert_eq!(result[&222], vec![3]);
      assert!(!result.contains_key(&999));
    }
  }

  mod resolve_location {
    use pretty_assertions::assert_eq;

    use super::*;

    fn space_row(location_id: i64) -> RawAssetRow {
      RawAssetRow {
        character_id: 1,
        is_blueprint_copy: None,
        is_singleton: false,
        item_id: 1,
        location_flag: "Hangar".to_string(),
        location_id,
        location_type: "space".to_string(),
        quantity: 1,
        type_id: 1,
      }
    }

    fn maps_with_system(system_id: i32, name: &str) -> AssetMaps {
      let mut system_name_map = HashMap::new();
      system_name_map.insert(system_id, name.to_string());
      AssetMaps {
        cat_key_map: HashMap::new(),
        group_name_map: HashMap::new(),
        is_container_set: HashSet::new(),
        item_index: HashMap::new(),
        price_cache: HashMap::new(),
        station_map: HashMap::new(),
        structure_name_map: HashMap::new(),
        structure_name_only: HashMap::new(),
        structure_system_name_map: HashMap::new(),
        system_name_map,
        type_cat_map: HashMap::new(),
        type_group_map: HashMap::new(),
        type_name_map: HashMap::new(),
        type_volume_map: HashMap::new(),
      }
    }

    #[test]
    fn it_resolves_space_assets_to_solar_system_name() {
      let row = space_row(30000142);
      let maps = maps_with_system(30000142, "Jita");

      let (loc, sys) = resolve_location(&row, &maps);

      assert_eq!(loc, "Jita");
      assert_eq!(sys, "Jita");
    }

    #[test]
    fn it_falls_back_for_unknown_space_system() {
      let row = space_row(30000999);
      let maps = maps_with_system(30000142, "Jita");

      let (loc, sys) = resolve_location(&row, &maps);

      assert_eq!(loc, "System 30000999");
      assert_eq!(sys, "System 30000999");
    }
  }

  mod resolve_solar_system_location {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_system_name_for_both_fields() {
      let mut sys_map: HashMap<i32, String> = HashMap::new();
      sys_map.insert(30000142, "Jita".to_string());

      let (loc, sys) = resolve_solar_system_location(30000142, &sys_map);

      assert_eq!(loc, "Jita");
      assert_eq!(sys, "Jita");
    }

    #[test]
    fn it_falls_back_when_system_unknown() {
      let sys_map: HashMap<i32, String> = HashMap::new();

      let (loc, sys) = resolve_solar_system_location(30000999, &sys_map);

      assert_eq!(loc, "System 30000999");
      assert_eq!(sys, "System 30000999");
    }
  }

  mod resolve_structure_location {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_name_and_system() {
      let mut name_map: HashMap<i64, (String, i64)> = HashMap::new();
      name_map.insert(1_000_000_000_001, ("Citadel Alpha".to_string(), 30000142));
      let mut sys_map: HashMap<i32, String> = HashMap::new();
      sys_map.insert(30000142, "Jita".to_string());

      let (name, sys) = resolve_structure_location(1_000_000_000_001, &name_map, &sys_map);

      assert_eq!(name, "Citadel Alpha");
      assert_eq!(sys, "Jita");
    }

    #[test]
    fn it_falls_back_for_unknown_structure() {
      let name_map: HashMap<i64, (String, i64)> = HashMap::new();
      let sys_map: HashMap<i32, String> = HashMap::new();

      let (name, sys) = resolve_structure_location(1_000_000_000_099, &name_map, &sys_map);

      assert_eq!(name, "Location 1000000000099");
      assert_eq!(sys, "");
    }
  }
}
