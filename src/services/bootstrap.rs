//! Application startup bootstrap sequence.

use std::path::PathBuf;

use chrono::Utc;
use iced::{Task, futures::SinkExt as _};
use pod_esi::Client;
use pod_model::{Character, CharacterAsset, CharacterAttributes, NeuralAttributes};
use tracing::Instrument as _;

use crate::services::character as character_service;

type Tx = iced::futures::channel::mpsc::Sender<Message>;

/// Bootstrap step messages surfaced to the splash screen.
#[derive(Clone, Debug)]
pub enum Message {
  /// Bootstrap completed; carries the open DB, loaded characters, and ESI client.
  Complete(pod_db::Repo, Vec<Character>, Option<pod_esi::Client>),
  /// A non-fatal error description.
  Error(String),
  /// DB is open but empty; static data must be seeded before continuing.
  SeedingRequired(pod_db::Repo),
  /// Static-data seeding finished; resume the normal post-DB flow.
  SeedingComplete(pod_db::Repo),
  /// Step label has changed; carries the text to show on the splash screen.
  StepChanged(String),
  /// ESI character sync started; carries total step count for proportional progress math.
  SyncStarted(usize),
}

/// Continues after the database is ready: creates ESI client, loads characters, streams ESI sync.
#[tracing::instrument(skip(db))]
pub fn continue_after_db(db: pod_db::Repo) -> Task<Message> {
  let (tx, rx) = iced::futures::channel::mpsc::channel(64);
  tokio::spawn(run_continue(db, tx));
  Task::stream(rx)
}

/// Kicks off a streaming ESI sync for the given characters.
#[allow(dead_code)]
pub fn sync_characters(db: pod_db::Repo, esi_client: pod_esi::Client, characters: Vec<Character>) -> Task<Message> {
  let (tx, rx) = iced::futures::channel::mpsc::channel(64);
  tokio::spawn(async move {
    let mut tx = tx;
    run_esi_sync(db, esi_client, characters, &mut tx).await;
  });
  Task::stream(rx)
}

/// Asynchronously opens the Pod SQLite database, runs pending migrations, then
/// always emits `SeedingRequired` so the SDE version check runs on every startup.
pub fn run() -> Task<Message> {
  Task::perform(
    async { open_database().await.map_err(|e| e.to_string()) },
    |result| match result {
      Ok(db) => Message::SeedingRequired(db),
      Err(e) => Message::Error(format!("Database error: {e}")),
    },
  )
}

async fn run_continue(db: pod_db::Repo, mut tx: Tx) {
  std::fs::create_dir_all(cache_path()).expect("failed to create cache directory");

  let esi = match Client::builder(crate::ESI_CLIENT_ID).disk_cache(cache_path()).build() {
    Ok(c) => c,
    Err(e) => {
      let _ = tx.send(Message::Error(e.to_string())).await;
      return;
    }
  };

  let characters = match db.characters().all().await {
    Ok(c) => c,
    Err(e) => {
      let _ = tx.send(Message::Error(e.to_string())).await;
      return;
    }
  };

  run_esi_sync(db, esi, characters, &mut tx).await;
}

async fn run_esi_sync(db: pod_db::Repo, esi: pod_esi::Client, mut characters: Vec<Character>, tx: &mut Tx) {
  if characters.is_empty() {
    let _ = tx.send(Message::Complete(db, characters, Some(esi))).await;
    return;
  }

  let total_steps = characters.len() * 9 + 1;
  let _ = tx.send(Message::SyncStarted(total_steps)).await;

  for character in characters.iter_mut() {
    let name = character.name().clone();
    let char_id = *character.id();
    let span = tracing::info_span!("sync_character", character_id = char_id);
    async {
      // Step 1: token refresh
      step(tx, format!("Refreshing token for {name}\u{2026}")).await;
      let token = match character_service::ensure_valid_token(character, &esi, &db).await {
        Some(t) => t,
        None => {
          tracing::warn!("bootstrap: skipping {name} — token refresh failed");
          for _ in 0..8 {
            step(tx, format!("Skipping {name}\u{2026}")).await;
          }
          return;
        }
      };
      // Keep the in-memory character in sync so downstream async tasks that call
      // ensure_valid_token don't try to re-use an already-rotated refresh token.
      character.set_access_token(token.clone());
      character.set_token_expires_at(chrono::Utc::now().timestamp() + 1199);
      let grant = character_service::refresh_grant(character, &token);
      let char_client = esi.character(&grant);

      // Step 2: corp data
      step(tx, format!("Syncing corp data for {name}\u{2026}")).await;
      if *character.corp_id() > 0
        && let Ok(detail) = esi.corporation(*character.corp_id()).detail().await
      {
        character.set_corp_name(detail.name.clone());
        let _ = db
          .characters()
          .update_corp(char_id, *character.corp_id(), detail.name)
          .await;
      }

      // Step 3: skills snapshot
      step(tx, format!("Syncing skills for {name}\u{2026}")).await;
      if let Ok(esi_skills) = char_client.skills().await {
        let skills = character_service::build_character_skills(char_id, esi_skills.skills, vec![]);
        let _ = db.characters().upsert_skills(char_id, &skills).await;
        *character.skills_mut() = skills;
      }

      // Step 4: skill queue — reconcile with already-synced skills
      step(tx, format!("Syncing skill queue for {name}\u{2026}")).await;
      if let Ok(queue) = char_client.skill_queue().await {
        let merged = character_service::reconcile_skills(char_id, character.skills(), queue.clone());
        let tq = character_service::build_training_queue(&queue, &merged);
        let _ = db.characters().upsert_skills(char_id, &merged).await;
        *character.skills_mut() = merged;
        *character.training_queue_mut() = tq;
      }

      // Step 5: neural attributes
      step(tx, format!("Syncing attributes for {name}\u{2026}")).await;
      if let Ok(esi_attrs) = char_client.attributes().await {
        let db_attrs = NeuralAttributes {
          charisma: esi_attrs.charisma,
          intelligence: esi_attrs.intelligence,
          memory: esi_attrs.memory,
          perception: esi_attrs.perception,
          willpower: esi_attrs.willpower,
        };
        let _ = db.characters().update_neural_attributes(char_id, &db_attrs).await;
        character.set_attributes(CharacterAttributes {
          charisma: esi_attrs.charisma,
          intelligence: esi_attrs.intelligence,
          memory: esi_attrs.memory,
          perception: esi_attrs.perception,
          willpower: esi_attrs.willpower,
          bonus_remaps: esi_attrs.bonus_remaps.unwrap_or(0),
          last_remap_date: esi_attrs.last_remap_date,
          accrued_remap_cooldown_date: esi_attrs.accrued_remap_cooldown_date,
        });
      }

      // Step 6: wallet balance
      step(tx, format!("Syncing wallet for {name}\u{2026}")).await;
      if let Ok(balance) = char_client.wallet_balance().await {
        let _ = db.characters().update_wallet(char_id, Some(balance.0)).await;
        character.set_isk_balance(Some(balance.0));
      }

      // Step 7: assets
      step(tx, format!("Syncing assets for {name}\u{2026}")).await;
      if let Ok(raw) = char_client.assets().await {
        let assets: Vec<CharacterAsset> = raw
          .into_iter()
          .map(|a| CharacterAsset {
            item_id: a.item_id,
            character_id: char_id,
            type_id: a.type_id,
            location_id: a.location_id,
            location_type: a.location_type,
            location_flag: a.location_flag,
            quantity: a.quantity,
            is_singleton: a.is_singleton,
            is_blueprint_copy: a.is_blueprint_copy,
          })
          .collect();
        let keep_ids: Vec<i64> = assets.iter().map(|a| a.item_id).collect();
        let _ = db.characters().upsert_assets(char_id, &assets).await;
        let _ = db.characters().delete_stale_assets(char_id, &keep_ids).await;

        cache_structure_names_from_assets(&assets, character, &grant, &esi, &db).await;
      }

      // Step 8: clones & implants
      step(tx, format!("Syncing clones for {name}\u{2026}")).await;
      let (clones_res, implants_res) = tokio::join!(char_client.clones(), char_client.implants());
      sync_clones_to_db(char_id, clones_res, implants_res, &db).await;

      // Step 9: portrait
      step(tx, format!("Loading portrait for {name}\u{2026}")).await;
      if let Ok(bytes) = esi.images().character_portrait(char_id, 256).await {
        *character.portrait_data_mut() = Some(bytes);
      }
    }
    .instrument(span)
    .await;
  }

  step(tx, "Fetching Jita prices\u{2026}".to_string()).await;
  sync_prices(&db, &esi).await;

  resolve_skill_names(&mut characters, &esi).await;
  for character in &mut characters {
    let name_map: std::collections::HashMap<i32, String> = character
      .skills()
      .iter()
      .filter_map(|s| s.skill_name.as_ref().map(|n| (s.skill_id, n.clone())))
      .collect();
    for entry in character.training_queue_mut() {
      if entry.skill_name.is_none() {
        entry.skill_name = name_map.get(&entry.skill_id).cloned();
      }
    }
  }
  let _ = tx.send(Message::Complete(db, characters, Some(esi))).await;
}

fn cache_path() -> PathBuf {
  dir_spec::cache_home()
    .map(|path| path.join("pod"))
    .expect("failed to resolve cache directory")
}

fn db_path() -> PathBuf {
  dir_spec::data_home()
    .map(|path| path.join("pod").join("pod.db"))
    .expect("failed to resolve user data directory")
}

async fn open_database() -> Result<pod_db::Repo, String> {
  pod_db::open(&db_path()).await.map_err(|e| e.to_string())
}

async fn resolve_skill_names(characters: &mut Vec<Character>, esi: &pod_esi::Client) {
  use std::collections::{HashMap, HashSet};

  let ids: Vec<i64> = characters
    .iter()
    .flat_map(|c| c.skills().iter().map(|s| s.skill_id as i64))
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();

  if ids.is_empty() {
    return;
  }

  let Ok(names) = esi.universe().names(&ids).await else {
    return;
  };

  let map: HashMap<i32, String> = names.into_iter().map(|n| (n.id as i32, n.name)).collect();
  for character in characters.iter_mut() {
    for skill in character.skills_mut() {
      if let Some(name) = map.get(&skill.skill_id) {
        skill.skill_name = Some(name.clone());
      }
    }
  }
}

async fn cache_structure_names_from_assets(
  assets: &[CharacterAsset],
  character: &Character,
  grant: &pod_esi::models::auth::Grant,
  esi: &Client,
  db: &pod_db::Repo,
) {
  use std::collections::HashSet;

  let structure_ids: Vec<i64> = assets
    .iter()
    .filter(|a| a.location_type != "item" && a.location_id >= i32::MAX as i64)
    .map(|a| a.location_id)
    .collect::<HashSet<_>>()
    .into_iter()
    .collect();

  if structure_ids.is_empty() {
    return;
  }

  let cached: std::collections::HashSet<i64> = db
    .universe()
    .structure_cache()
    .find_by_ids(&structure_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(id, _)| id)
    .collect();

  let mut resolved: Vec<(i64, String)> = Vec::new();
  for id in structure_ids {
    if cached.contains(&id) {
      continue;
    }
    if let Ok(info) = esi.universe().structure(id).auth(grant).detail().await {
      resolved.push((id, info.name));
    } else {
      tracing::debug!(
        "bootstrap: could not resolve structure {} for character {}",
        id,
        character.name()
      );
    }
  }

  if !resolved.is_empty() {
    let _ = db.universe().structure_cache().upsert_many(&resolved).await;
  }
}

async fn sync_clones_to_db(
  char_id: i64,
  clones_res: Result<pod_esi::models::character::Clones, pod_esi::Error>,
  implants_res: Result<Vec<i32>, pod_esi::Error>,
  db: &pod_db::Repo,
) {
  let clones_data = match clones_res {
    Ok(c) => c,
    Err(e) => {
      tracing::warn!("bootstrap: failed to fetch clones for character {char_id}: {e}");
      return;
    }
  };
  let active_implant_ids = implants_res.unwrap_or_default();

  let mut all_type_ids: Vec<i32> = active_implant_ids.clone();
  for jc in &clones_data.jump_clones {
    all_type_ids.extend_from_slice(&jc.implants);
  }
  all_type_ids.sort_unstable();
  all_type_ids.dedup();

  let implant_rows = db
    .universe()
    .item_types()
    .implant_data_for_ids(&all_type_ids)
    .await
    .unwrap_or_default();

  let data_map: std::collections::HashMap<i32, (String, i32, String)> = implant_rows
    .into_iter()
    .map(|(tid, bonus, slot, name)| (tid, (bonus, slot, name)))
    .collect();

  let now = chrono::Utc::now().to_rfc3339();

  let build_implants = |clone_id: i64, type_ids: &[i32]| -> Vec<pod_db::StartupImplant> {
    type_ids
      .iter()
      .enumerate()
      .map(|(i, &tid)| {
        let (bonus, slot, name) = data_map
          .get(&tid)
          .cloned()
          .unwrap_or_else(|| ("{}".to_string(), (i + 1) as i32, tid.to_string()));
        pod_db::StartupImplant {
          clone_id,
          type_id: tid,
          slot,
          name,
          attribute_bonus: bonus,
        }
      })
      .collect()
  };

  let home_loc_id = clones_data
    .home_location
    .as_ref()
    .and_then(|l| l.location_id)
    .unwrap_or(0);
  let active_clone = pod_db::StartupClone {
    character_id: char_id,
    id: 0,
    installed_at: None,
    is_active: true,
    location_id: home_loc_id,
    name: None,
    synced_at: now.clone(),
  };
  let active_implants = build_implants(0, &active_implant_ids);

  let mut clone_data: Vec<(pod_db::StartupClone, Vec<pod_db::StartupImplant>)> = vec![(active_clone, active_implants)];

  for jc in clones_data.jump_clones {
    let Some(clone_id) = jc.clone_id else {
      continue;
    };
    let clone = pod_db::StartupClone {
      character_id: char_id,
      id: clone_id,
      installed_at: clones_data.last_station_change_date.clone(),
      is_active: false,
      location_id: jc.location_id,
      name: jc.name,
      synced_at: now.clone(),
    };
    let implants = build_implants(clone_id, &jc.implants);
    clone_data.push((clone, implants));
  }

  let _ = db.clones().upsert_startup_clones(&clone_data).await;
}

#[tracing::instrument(skip(db, esi))]
async fn sync_prices(db: &pod_db::Repo, esi: &pod_esi::Client) {
  let today = Utc::now().date_naive();

  if let Ok(dates) = db.prices().dates_needing_aggregation(today).await {
    for date in dates {
      let _ = db.prices().aggregate_and_prune(date).await;
    }
  }

  let type_ids = match db.prices().types_to_track().await {
    Ok(ids) => ids,
    Err(e) => {
      tracing::warn!("bootstrap: failed to get types to track: {e}");
      return;
    }
  };

  if type_ids.is_empty() {
    return;
  }

  let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(10));

  let mut handles = Vec::with_capacity(type_ids.len());
  for type_id in type_ids {
    let permit = semaphore.clone().acquire_owned().await.expect("semaphore closed");
    let esi = esi.clone();
    let db = db.clone();
    handles.push(tokio::spawn(async move {
      let _permit = permit;
      let now = Utc::now();
      match esi.markets().lowest_jita_sell(type_id).await {
        Ok(Some(price)) => {
          let _ = db.prices().insert_price(type_id, price, now).await;
        }
        Ok(None) => {}
        Err(e) => {
          tracing::warn!("bootstrap: price fetch failed for type {type_id}: {e}");
        }
      }
    }));
  }

  for handle in handles {
    let _ = handle.await;
  }
}

async fn step(tx: &mut Tx, label: String) {
  let _ = tx.send(Message::StepChanged(label)).await;
}
