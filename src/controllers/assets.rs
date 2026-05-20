//! Assets controller: startup data fetch from the database.

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use pod_model::{Character, Corporation, Station};
use pod_ui::views::assets::{
  self, AssetRecord, AssetValuesData, CategoryValue, CharacterStructureCell, Message, State, StockpileItemStatus,
  StockpileWithStatus, TopItem,
};

use crate::services::{Services, character as character_service, corporation as corporation_service};

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
  loop {
    let Some((loc_id, loc_type, _, _)) = item_index.get(&cursor_id) else {
      break;
    };
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
      // cursor_id is not in item_index — it's a player structure terminus
      if cursor_id == parent_item_id {
        break;
      }
      let structure_name = structure_name_map
        .get(&cursor_id)
        .cloned()
        .unwrap_or_else(|| format!("Location {}", cursor_id));
      let flag = humanize_flag(&last_flag);
      let container_type = type_name_map
        .get(&last_type_id)
        .map(|n| n.as_str())
        .unwrap_or("Container");
      let path = format!("{} · {} · {}", structure_name, flag, container_type);
      return (path, last_container_id);
    };
    if loc_type != "item" || depth > 20 {
      let station_name = if loc_type == "station" && *loc_id < i32::MAX as i64 {
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
      };
      let flag = humanize_flag(loc_flag);
      let container_type = type_name_map.get(type_id).map(|n| n.as_str()).unwrap_or("Container");
      let path = format!("{} · {} · {}", station_name, flag, container_type);
      return (path, cursor_id);
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

  let unique_ids: Vec<i64> = locs
    .iter()
    .map(|(id, _)| *id)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();

  let mut result: HashMap<i64, (String, i64)> = db
    .universe()
    .structure_cache()
    .find_by_ids(&unique_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(id, name)| (id, (name, 0i64)))
    .collect();

  let missing: Vec<i64> = unique_ids.into_iter().filter(|id| !result.contains_key(id)).collect();
  if missing.is_empty() {
    return result;
  }

  let Some(esi) = esi else {
    return result;
  };

  let char_map: HashMap<i64, &Character> = characters.iter().map(|c| (*c.id(), c)).collect();
  let mut struct_chars: HashMap<i64, Vec<i64>> = HashMap::new();
  for &(loc_id, char_id) in locs {
    if missing.contains(&loc_id) {
      struct_chars.entry(loc_id).or_default().push(char_id);
    }
  }

  let mut newly_resolved: Vec<(i64, String)> = Vec::new();
  for (struct_id, char_ids) in &struct_chars {
    let mut resolved = false;
    for &char_id in char_ids {
      let Some(character) = char_map.get(&char_id) else {
        continue;
      };
      let Some(token) = character_service::ensure_valid_token(character, esi, db).await else {
        tracing::warn!("assets: token unavailable for character {char_id}, skipping structure {struct_id}");
        continue;
      };
      let grant = character_service::refresh_grant(character, &token);
      match esi.universe().structure(*struct_id).auth(&grant).detail().await {
        Ok(info) => {
          newly_resolved.push((*struct_id, info.name.clone()));
          result.insert(*struct_id, (info.name, info.solar_system_id));
          resolved = true;
          break;
        }
        Err(e) => {
          tracing::warn!("assets: ESI structure {struct_id} failed for character {char_id}: {e}");
        }
      }
    }
    if !resolved {
      tracing::warn!("assets: could not resolve structure {struct_id} — will show as Location ID");
    }
  }

  if !newly_resolved.is_empty() {
    let _ = db.universe().structure_cache().upsert_many(&newly_resolved).await;
  }

  result
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

  let type_ids: Vec<i32> = asset_rows
    .iter()
    .map(|a| a.type_id)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  let type_map: HashMap<i32, _> = db
    .universe()
    .item_types()
    .find_by_ids(&type_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.id, r))
    .collect();

  let group_ids: Vec<i32> = type_map
    .values()
    .map(|t| t.item_group_id)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  let group_map: HashMap<i32, _> = db
    .universe()
    .item_groups()
    .find_by_ids(&group_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.id, r))
    .collect();

  let cat_ids: Vec<i32> = group_map
    .values()
    .map(|g| g.item_category_id)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  let cat_map: HashMap<i32, _> = db
    .universe()
    .item_categories()
    .find_by_ids(&cat_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.id, r))
    .collect();

  let station_location_ids: Vec<i32> = asset_rows
    .iter()
    .filter(|a| a.location_type == "station" && a.location_id < i32::MAX as i64)
    .map(|a| a.location_id as i32)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();

  let mut station_map: HashMap<i32, Station> = db
    .universe()
    .stations()
    .find_by_ids(&station_location_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|s| (*s.id(), s))
    .collect();

  if let Some(ref esi) = esi {
    let missing: Vec<i32> = station_location_ids
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

  let sys_ids: Vec<i32> = station_map
    .values()
    .map(|s| *s.solar_system_id())
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  let system_map: HashMap<i32, _> = db
    .universe()
    .solar_systems()
    .find_by_ids(&sys_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.id, r))
    .collect();

  let type_name_map: HashMap<i32, String> = type_map.iter().map(|(&id, t)| (id, t.name.clone())).collect();
  let system_name_map: HashMap<i32, String> = system_map.iter().map(|(&id, s)| (id, s.name.clone())).collect();
  let item_index: HashMap<i64, (i64, String, String, i32)> = asset_rows
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
    .collect();

  let mut is_container_set: HashSet<i64> = HashSet::new();
  for (loc_id, loc_type, _, _) in item_index.values() {
    if loc_type == "item" {
      is_container_set.insert(*loc_id);
    }
  }

  let structure_locs: Vec<(i64, i64)> = asset_rows
    .iter()
    .filter(|a| a.location_id >= i32::MAX as i64 && !item_index.contains_key(&a.location_id))
    .map(|a| (a.location_id, a.character_id))
    .collect();
  let structure_name_map = resolve_structure_names(&structure_locs, &characters, esi.as_ref(), &db).await;

  let structure_solar_sys_ids: Vec<i32> = structure_name_map
    .values()
    .filter_map(|(_, sys_id)| if *sys_id > 0 { i32::try_from(*sys_id).ok() } else { None })
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  let structure_system_map: HashMap<i32, String> = if structure_solar_sys_ids.is_empty() {
    HashMap::new()
  } else {
    db.universe()
      .solar_systems()
      .find_by_ids(&structure_solar_sys_ids)
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|s| (s.id, s.name))
      .collect()
  };

  let structure_name_only: HashMap<i64, String> = structure_name_map
    .iter()
    .map(|(&id, (name, _))| (id, name.clone()))
    .collect();

  let prices_repo = db.prices();
  let mut price_cache: HashMap<i32, f64> = HashMap::new();
  for &type_id in type_map.keys() {
    if let Ok(Some(price)) = prices_repo.latest_price(type_id).await {
      price_cache.insert(type_id, price);
    }
  }

  asset_rows
    .into_iter()
    .map(|a| {
      let item_type = type_map.get(&a.type_id);
      let type_name = item_type
        .map(|t| t.name.clone())
        .unwrap_or_else(|| format!("Type {}", a.type_id));
      let group = item_type.and_then(|t| group_map.get(&t.item_group_id));
      let group_name = group.map(|g| g.name.clone()).unwrap_or_default();
      let category = group.and_then(|g| cat_map.get(&g.item_category_id));
      let category_key = category
        .map(|c| category_name_to_key(&c.name))
        .unwrap_or("commodity")
        .to_string();
      let volume = item_type.and_then(|t| t.packaged_volume.or(t.volume)).unwrap_or(0.0);
      let unit_price = price_cache.get(&a.type_id).copied().unwrap_or(0.0);

      let is_at_structure = a.location_id >= i32::MAX as i64 && !item_index.contains_key(&a.location_id);

      let (location_name, system_name) = if a.location_type == "station" && a.location_id < i32::MAX as i64 {
        let station = station_map.get(&(a.location_id as i32));
        let loc = station
          .map(|s| s.name().clone())
          .unwrap_or_else(|| format!("Station {}", a.location_id));
        let sys = station
          .and_then(|s| system_map.get(s.solar_system_id()))
          .map(|s| s.name.clone())
          .unwrap_or_default();
        (loc, sys)
      } else if a.location_type == "solar_system" && a.location_id < i32::MAX as i64 {
        let sys = system_map
          .get(&(a.location_id as i32))
          .map(|s| s.name.clone())
          .unwrap_or_else(|| format!("System {}", a.location_id));
        (sys.clone(), sys)
      } else if is_at_structure {
        let (name, solar_sys_id) = structure_name_map
          .get(&a.location_id)
          .map(|(n, s)| (n.clone(), *s))
          .unwrap_or_else(|| (format!("Location {}", a.location_id), 0));
        let sys = if solar_sys_id > 0 {
          i32::try_from(solar_sys_id)
            .ok()
            .and_then(|id| structure_system_map.get(&id).cloned())
            .unwrap_or_default()
        } else {
          String::new()
        };
        (name, sys)
      } else {
        (String::new(), String::new())
      };

      let (container_path, container_id) = if a.location_type == "item" && !is_at_structure {
        resolve_container_path(
          a.location_id,
          &item_index,
          &type_name_map,
          &station_map,
          &system_name_map,
          &structure_name_only,
        )
      } else {
        (String::new(), 0)
      };

      let depth = compute_depth(a.item_id, &item_index);
      let is_container = is_container_set.contains(&a.item_id);

      AssetRecord {
        item_id: a.item_id,
        character_id: a.character_id,
        type_id: a.type_id,
        type_name,
        group_name,
        category_key,
        unit_price,
        volume,
        quantity: a.quantity as i64,
        location_id: a.location_id,
        location_name,
        system_name,
        is_singleton: a.is_singleton,
        container_path,
        container_id,
        depth,
        icon_variant: icon_variant(a.is_blueprint_copy).to_string(),
        is_container,
      }
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
  let corp_esi = esi.corporation(corp_id);
  let corp_client = corp_esi.auth(&grant);

  let asset_rows = match corp_client.assets().await {
    Ok(rows) => rows,
    Err(_) => return Vec::new(),
  };

  if asset_rows.is_empty() {
    return Vec::new();
  }

  let type_ids: Vec<i32> = asset_rows
    .iter()
    .map(|a| a.type_id)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  let type_map: HashMap<i32, _> = db
    .universe()
    .item_types()
    .find_by_ids(&type_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.id, r))
    .collect();

  let group_ids: Vec<i32> = type_map
    .values()
    .map(|t| t.item_group_id)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  let group_map: HashMap<i32, _> = db
    .universe()
    .item_groups()
    .find_by_ids(&group_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.id, r))
    .collect();

  let cat_ids: Vec<i32> = group_map
    .values()
    .map(|g| g.item_category_id)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  let cat_map: HashMap<i32, _> = db
    .universe()
    .item_categories()
    .find_by_ids(&cat_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.id, r))
    .collect();

  let station_location_ids: Vec<i32> = asset_rows
    .iter()
    .filter(|a| a.location_type == "station" && a.location_id < i32::MAX as i64)
    .map(|a| a.location_id as i32)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();

  let mut station_map: HashMap<i32, Station> = db
    .universe()
    .stations()
    .find_by_ids(&station_location_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|s| (*s.id(), s))
    .collect();

  let missing: Vec<i32> = station_location_ids
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

  let sys_ids: Vec<i32> = station_map
    .values()
    .map(|s| *s.solar_system_id())
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  let system_map: HashMap<i32, _> = db
    .universe()
    .solar_systems()
    .find_by_ids(&sys_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.id, r))
    .collect();

  let type_name_map: HashMap<i32, String> = type_map.iter().map(|(&id, t)| (id, t.name.clone())).collect();
  let system_name_map: HashMap<i32, String> = system_map.iter().map(|(&id, s)| (id, s.name.clone())).collect();
  let item_index: HashMap<i64, (i64, String, String, i32)> = asset_rows
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
    .collect();

  let mut is_container_set: HashSet<i64> = HashSet::new();
  for (loc_id, loc_type, _, _) in item_index.values() {
    if loc_type == "item" {
      is_container_set.insert(*loc_id);
    }
  }

  let corp_char_id = characters.first().map(|c| *c.id()).unwrap_or(corp_id);
  let structure_locs: Vec<(i64, i64)> = asset_rows
    .iter()
    .filter(|a| a.location_id >= i32::MAX as i64 && !item_index.contains_key(&a.location_id))
    .map(|a| (a.location_id, corp_char_id))
    .collect();
  let structure_name_map = resolve_structure_names(&structure_locs, &characters, Some(&esi), &db).await;

  let structure_solar_sys_ids: Vec<i32> = structure_name_map
    .values()
    .filter_map(|(_, sys_id)| if *sys_id > 0 { i32::try_from(*sys_id).ok() } else { None })
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();
  let structure_system_map: HashMap<i32, String> = if structure_solar_sys_ids.is_empty() {
    HashMap::new()
  } else {
    db.universe()
      .solar_systems()
      .find_by_ids(&structure_solar_sys_ids)
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|s| (s.id, s.name))
      .collect()
  };

  let structure_name_only: HashMap<i64, String> = structure_name_map
    .iter()
    .map(|(&id, (name, _))| (id, name.clone()))
    .collect();

  let prices_repo = db.prices();
  let mut price_cache: HashMap<i32, f64> = HashMap::new();
  for &type_id in type_map.keys() {
    if let Ok(Some(price)) = prices_repo.latest_price(type_id).await {
      price_cache.insert(type_id, price);
    }
  }

  asset_rows
    .into_iter()
    .map(|a| {
      let item_type = type_map.get(&a.type_id);
      let type_name = item_type
        .map(|t| t.name.clone())
        .unwrap_or_else(|| format!("Type {}", a.type_id));
      let group = item_type.and_then(|t| group_map.get(&t.item_group_id));
      let group_name = group.map(|g| g.name.clone()).unwrap_or_default();
      let category = group.and_then(|g| cat_map.get(&g.item_category_id));
      let category_key = category
        .map(|c| category_name_to_key(&c.name))
        .unwrap_or("commodity")
        .to_string();
      let volume = item_type.and_then(|t| t.packaged_volume.or(t.volume)).unwrap_or(0.0);
      let unit_price = price_cache.get(&a.type_id).copied().unwrap_or(0.0);

      let is_at_structure = a.location_id >= i32::MAX as i64 && !item_index.contains_key(&a.location_id);

      let (location_name, system_name) = if a.location_type == "station" && a.location_id < i32::MAX as i64 {
        let station = station_map.get(&(a.location_id as i32));
        let loc = station
          .map(|s| s.name().clone())
          .unwrap_or_else(|| format!("Station {}", a.location_id));
        let sys = station
          .and_then(|s| system_map.get(s.solar_system_id()))
          .map(|s| s.name.clone())
          .unwrap_or_default();
        (loc, sys)
      } else if a.location_type == "solar_system" && a.location_id < i32::MAX as i64 {
        let sys = system_map
          .get(&(a.location_id as i32))
          .map(|s| s.name.clone())
          .unwrap_or_else(|| format!("System {}", a.location_id));
        (sys.clone(), sys)
      } else if is_at_structure {
        let (name, solar_sys_id) = structure_name_map
          .get(&a.location_id)
          .map(|(n, s)| (n.clone(), *s))
          .unwrap_or_else(|| (format!("Location {}", a.location_id), 0));
        let sys = if solar_sys_id > 0 {
          i32::try_from(solar_sys_id)
            .ok()
            .and_then(|id| structure_system_map.get(&id).cloned())
            .unwrap_or_default()
        } else {
          String::new()
        };
        (name, sys)
      } else {
        (String::new(), String::new())
      };

      let (container_path, container_id) = if a.location_type == "item" && !is_at_structure {
        resolve_container_path(
          a.location_id,
          &item_index,
          &type_name_map,
          &station_map,
          &system_name_map,
          &structure_name_only,
        )
      } else {
        (String::new(), 0)
      };

      let depth = compute_depth(a.item_id, &item_index);
      let is_container = is_container_set.contains(&a.item_id);

      AssetRecord {
        item_id: a.item_id,
        character_id: corp_id,
        type_id: a.type_id,
        type_name,
        group_name,
        category_key,
        unit_price,
        volume,
        quantity: a.quantity as i64,
        location_id: a.location_id,
        location_name,
        system_name,
        is_singleton: a.is_singleton,
        container_path,
        container_id,
        depth,
        icon_variant: icon_variant(a.is_blueprint_copy).to_string(),
        is_container,
      }
    })
    .collect()
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
    let items: Vec<StockpileItemStatus> = statuses
      .into_iter()
      .map(|s| {
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
      })
      .collect();
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

pub async fn nav_history(db: pod_db::Repo, char_ids: Vec<i64>, days: u32) -> Vec<(NaiveDate, f64)> {
  db.prices().nav_history(&char_ids, days).await.unwrap_or_default()
}

/// Computes the full asset values breakdown for the Values tab.
pub async fn asset_values_breakdown(
  assets: Vec<AssetRecord>,
  characters: Vec<pod_model::Character>,
  db: pod_db::Repo,
) -> AssetValuesData {
  let prices_repo = db.prices();

  let type_ids: Vec<i32> = assets
    .iter()
    .map(|a| a.type_id)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();

  let mut price_cache: HashMap<i32, f64> = HashMap::new();
  for type_id in type_ids {
    if let Ok(Some(price)) = prices_repo.latest_price(type_id).await {
      price_cache.insert(type_id, price);
    }
  }

  let char_name_map: HashMap<i64, String> = characters.iter().map(|c| (*c.id(), c.name().clone())).collect();

  let valued: Vec<(&AssetRecord, f64)> = assets
    .iter()
    .map(|a| {
      let unit_price = price_cache.get(&a.type_id).copied().unwrap_or(0.0);
      let value = unit_price * a.quantity as f64;
      (a, value)
    })
    .collect();

  let total_value: f64 = valued.iter().map(|(_, v)| v).sum();

  let mut matrix: HashMap<(i64, String), (String, String, f64)> = HashMap::new();
  for (a, value) in &valued {
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

  let mut character_structure_cells: Vec<CharacterStructureCell> = matrix
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
  character_structure_cells.sort_by(|a, b| {
    a.character_id
      .cmp(&b.character_id)
      .then_with(|| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal))
  });

  let mut cat_map: HashMap<String, f64> = HashMap::new();
  for (a, value) in &valued {
    let cat = if a.category_key == "all" || a.category_key.is_empty() {
      "commodity"
    } else {
      &a.category_key
    };
    *cat_map.entry(cat.to_string()).or_insert(0.0) += value;
  }
  let mut category_breakdown: Vec<CategoryValue> = cat_map
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
  category_breakdown.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));

  let mut top_map: HashMap<i32, (String, String, i64, f64)> = HashMap::new();
  for (a, value) in &valued {
    let entry = top_map
      .entry(a.type_id)
      .or_insert_with(|| (a.type_name.clone(), a.category_key.clone(), 0, 0.0));
    entry.2 += a.quantity;
    entry.3 += value;
  }
  let mut top_items: Vec<TopItem> = top_map
    .into_iter()
    .map(|(type_id, (type_name, category_name, total_quantity, value))| TopItem {
      type_id,
      type_name,
      category_name,
      total_quantity,
      value,
    })
    .collect();
  top_items.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
  top_items.truncate(10);

  AssetValuesData {
    character_structure_cells,
    category_breakdown,
    top_items,
    total_value,
  }
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
