//! Application startup bootstrap sequence.

use std::path::PathBuf;

use iced::{Task, futures::SinkExt as _};
use pod_esi::Client;
use pod_model::{Character, CharacterAsset, CharacterAttributes, CorporationAsset, NeuralAttributes};
use tracing::Instrument as _;

use crate::services::{character as character_service, corporation as corporation_service};

type Tx = iced::futures::channel::mpsc::Sender<Message>;

/// Bootstrap step messages surfaced to the splash screen and post-launch sync.
#[derive(Clone, Debug)]
pub enum Message {
  /// A character's background sync completed; carries the updated character.
  CharacterSynced(Character),
  /// Bootstrap completed; carries the open DB, loaded characters, and ESI client.
  Complete(pod_db::Repo, Vec<Character>, Option<pod_esi::Client>),
  /// A non-fatal error description.
  Error(String),
  /// A fatal error that requires the app to exit.
  FatalError(String),
  /// Static-data seeding finished; resume the normal post-DB flow.
  SeedingComplete(pod_db::Repo),
  /// DB is open but empty; static data must be seeded before continuing.
  SeedingRequired(pod_db::Repo),
  /// Step label has changed; carries the text to show on the splash screen.
  StepChanged(String),
  /// Token refresh failed for the given character ID; surface a re-auth prompt.
  #[allow(dead_code)]
  TokenRefreshFailed(i64),
}

/// Continues after the database is ready: creates the ESI client, loads characters from DB,
/// then emits `Complete` without making any ESI network calls.
#[tracing::instrument(skip(db))]
pub fn continue_after_db(db: pod_db::Repo) -> Task<Message> {
  let (tx, rx) = iced::futures::channel::mpsc::channel(64);
  tokio::spawn(run_minimal_boot(db, tx));
  Task::stream(rx)
}

/// Kicks off a parallel background ESI sync for the given characters.
///
/// Each character syncs independently via a spawned task. Results are streamed back as
/// `CharacterSynced` or `TokenRefreshFailed` messages as each finishes. Price sync runs
/// after all characters complete.
pub fn sync_characters(db: pod_db::Repo, esi_client: Client, characters: Vec<Character>) -> Task<Message> {
  let (tx, rx) = iced::futures::channel::mpsc::channel(64);
  tokio::spawn(run_background_sync(db, esi_client, characters, tx));
  Task::stream(rx)
}

/// Asynchronously opens the Pod SQLite database, runs pending migrations, then
/// always emits `SeedingRequired` so the SDE version check runs on every startup.
pub fn run() -> Task<Message> {
  Task::perform(
    async { open_database().await.map_err(|e| e.to_string()) },
    |result| match result {
      Ok(db) => Message::SeedingRequired(db),
      Err(e) => Message::FatalError(format!("Database error: {e}")),
    },
  )
}

async fn run_minimal_boot(db: pod_db::Repo, mut tx: Tx) {
  if let Err(e) = std::fs::create_dir_all(cache_path()) {
    let _ = tx
      .send(Message::Error(format!("failed to create cache directory: {e}")))
      .await;
    return;
  }

  step(&mut tx, "Building ESI client\u{2026}".to_string()).await;
  let esi = match Client::builder(crate::ESI_CLIENT_ID).disk_cache(cache_path()).build() {
    Ok(c) => c,
    Err(e) => {
      let _ = tx.send(Message::Error(e.to_string())).await;
      return;
    }
  };

  step(&mut tx, "Loading characters\u{2026}".to_string()).await;
  if let Err(e) = db.characters().normalize_sort_orders().await {
    let _ = tx.send(Message::Error(e.to_string())).await;
  }
  let mut characters = match db.characters().all().await {
    Ok(c) => c,
    Err(e) => {
      let _ = tx.send(Message::Error(e.to_string())).await;
      return;
    }
  };

  for character in &mut characters {
    if let Some(bytes) = crate::services::portraits::load(*character.id()) {
      *character.portrait_data_mut() = Some(bytes);
    }
  }

  let _ = tx.send(Message::Complete(db, characters, Some(esi))).await;
}

async fn run_background_sync(db: pod_db::Repo, esi: Client, characters: Vec<Character>, mut tx: Tx) {
  if characters.is_empty() {
    crate::services::prices::sync(&db, &esi).await;
    return;
  }

  let (result_tx, mut result_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

  for character in characters {
    let db = db.clone();
    let esi = esi.clone();
    let result_tx = result_tx.clone();
    tokio::spawn(async move {
      let msg = sync_one_character(character, esi, db).await;
      let _ = result_tx.send(msg);
    });
  }
  drop(result_tx);

  while let Some(msg) = result_rx.recv().await {
    let _ = tx.send(msg).await;
  }

  crate::services::prices::sync(&db, &esi).await;
}

async fn sync_one_character(mut character: Character, esi: Client, db: pod_db::Repo) -> Message {
  let char_id = *character.id();
  let name = character.name().clone();
  let span = tracing::info_span!("sync_character", character_id = char_id);

  async {
    let token = match character_service::ensure_valid_token(&character, &esi, &db).await {
      Some(t) => t,
      None => {
        tracing::warn!("background sync: skipping {name} — token refresh failed");
        return Message::TokenRefreshFailed(char_id);
      }
    };
    character.set_access_token(token.clone());
    character.set_token_expires_at(chrono::Utc::now().timestamp() + 1199);
    character_service::backfill_granted_scopes(&mut character, &token, &db).await;
    let grant = character_service::refresh_grant(&character, &token);

    sync_corp_data(&mut character, char_id, &esi, &db).await;
    sync_skills_and_queue(&mut character, char_id, &esi, &grant, &db).await;
    sync_neural_attributes(&mut character, char_id, &esi, &grant, &db).await;
    sync_wallet(&mut character, char_id, &esi, &grant, &db).await;
    sync_assets(&mut character, char_id, &esi, &grant, &db).await;
    sync_active_ship(&character, char_id, &esi, &grant, &db).await;
    sync_corp_assets(&character, &esi, &db).await;

    let char_client = esi.character(&grant);
    let (clones_res, implants_res) = tokio::join!(char_client.clones(), char_client.implants());
    sync_clones_to_db(char_id, clones_res, implants_res, &db).await;

    if let Ok(bytes) = esi.images().character_portrait(char_id, 256).await {
      crate::services::portraits::save(char_id, &bytes);
      *character.portrait_data_mut() = Some(bytes);
    }

    propagate_skill_names(&mut character, &esi).await;

    Message::CharacterSynced(character)
  }
  .instrument(span)
  .await
}

async fn sync_corp_data(character: &mut Character, char_id: i64, esi: &Client, db: &pod_db::Repo) {
  if *character.corp_id() > 0
    && let Ok(detail) = esi.corporation(*character.corp_id()).detail().await
  {
    character.set_corp_name(detail.name.clone());
    let _ = db
      .characters()
      .update_corp(char_id, *character.corp_id(), detail.name)
      .await;
  }
}

async fn sync_skills_and_queue(
  character: &mut Character,
  char_id: i64,
  esi: &Client,
  grant: &pod_esi::models::auth::Grant,
  db: &pod_db::Repo,
) {
  let char_client = esi.character(grant);
  if let Ok(esi_skills) = char_client.skills().await {
    let skills = character_service::build_character_skills(char_id, esi_skills.skills, vec![]);
    let _ = db.characters().upsert_skills(char_id, &skills).await;
    *character.skills_mut() = skills;
  }
  if let Ok(queue) = char_client.skill_queue().await {
    let merged = character_service::reconcile_skills(char_id, character.skills(), queue.clone());
    let tq = character_service::build_training_queue(&queue, &merged);
    let _ = db.characters().upsert_skills(char_id, &merged).await;
    *character.skills_mut() = merged;
    *character.training_queue_mut() = tq;
  }
}

async fn sync_neural_attributes(
  character: &mut Character,
  char_id: i64,
  esi: &Client,
  grant: &pod_esi::models::auth::Grant,
  db: &pod_db::Repo,
) {
  let char_client = esi.character(grant);
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
      accrued_remap_cooldown_date: esi_attrs.accrued_remap_cooldown_date,
      bonus_remaps: esi_attrs.bonus_remaps.unwrap_or(0),
      charisma: esi_attrs.charisma,
      intelligence: esi_attrs.intelligence,
      last_remap_date: esi_attrs.last_remap_date,
      memory: esi_attrs.memory,
      perception: esi_attrs.perception,
      willpower: esi_attrs.willpower,
    });
  }
}

async fn sync_wallet(
  character: &mut Character,
  char_id: i64,
  esi: &Client,
  grant: &pod_esi::models::auth::Grant,
  db: &pod_db::Repo,
) {
  let char_client = esi.character(grant);
  if let Ok(balance) = char_client.wallet_balance().await {
    let _ = db.characters().update_wallet(char_id, Some(balance.0)).await;
    character.set_isk_balance(Some(balance.0));
  }
}

async fn sync_assets(
  character: &mut Character,
  char_id: i64,
  esi: &Client,
  grant: &pod_esi::models::auth::Grant,
  db: &pod_db::Repo,
) {
  let char_client = esi.character(grant);
  if let Ok(raw) = char_client.assets().await {
    let assets: Vec<CharacterAsset> = raw
      .into_iter()
      .map(|a| CharacterAsset {
        character_id: char_id,
        is_blueprint_copy: a.is_blueprint_copy,
        is_singleton: a.is_singleton,
        item_id: a.item_id,
        location_flag: a.location_flag,
        location_id: a.location_id,
        location_type: a.location_type,
        quantity: a.quantity,
        type_id: a.type_id,
        ..Default::default()
      })
      .collect();
    let keep_ids: Vec<i64> = assets.iter().map(|a| a.item_id).collect();
    let _ = db.characters().upsert_assets(char_id, &assets).await;
    let _ = db.characters().delete_stale_assets(char_id, &keep_ids).await;
    cache_structure_names_from_assets(&assets, character, grant, esi, db).await;
  }
}

async fn sync_active_ship(
  character: &Character,
  char_id: i64,
  esi: &Client,
  grant: &pod_esi::models::auth::Grant,
  db: &pod_db::Repo,
) {
  let char_client = esi.character(grant);
  let (ship_res, location_res) = tokio::join!(char_client.ship(), char_client.location());
  let (ship, location) = match (ship_res, location_res) {
    (Ok(s), Ok(l)) => (s, l),
    (Err(e), _) => {
      tracing::warn!(
        "bootstrap: failed to fetch ship for character {}: {e}",
        character.name()
      );
      return;
    }
    (_, Err(e)) => {
      tracing::warn!(
        "bootstrap: failed to fetch location for character {}: {e}",
        character.name()
      );
      return;
    }
  };

  let (location_id, location_type) = if let Some(id) = location.station_id {
    (id, "station")
  } else if let Some(id) = location.structure_id {
    (id, "item")
  } else {
    (location.solar_system_id, "solar_system")
  };

  let synthetic = CharacterAsset {
    character_id: char_id,
    is_active_ship: true,
    is_blueprint_copy: None,
    is_singleton: true,
    item_id: ship.ship_item_id,
    location_flag: "Active Ship".to_string(),
    location_id,
    location_type: location_type.to_string(),
    quantity: 1,
    ship_name: Some(ship.ship_name),
    type_id: ship.ship_type_id,
  };

  let _ = db.characters().upsert_assets(char_id, &[synthetic]).await;
}

async fn sync_corp_assets(character: &Character, esi: &Client, db: &pod_db::Repo) {
  let all_corps = match db.corporations().all().await {
    Ok(corps) => corps,
    Err(e) => {
      tracing::warn!("background sync: failed to load corporations: {e}");
      return;
    }
  };

  let now = chrono::Utc::now().timestamp();

  for corp in all_corps
    .into_iter()
    .filter(|c| *c.auth_character_id() == *character.id())
  {
    let corp_id = *corp.id();

    if let Ok(Some(state)) = db.assets().get_asset_sync_state("corporation", corp_id).await
      && let Some(expires_at) = state.cache_expires_at
      && expires_at > now
    {
      continue;
    }

    let token = match corporation_service::ensure_valid_token(&corp, esi, db).await {
      Some(t) => t,
      None => {
        tracing::warn!("background sync: skipping corp {corp_id} — token refresh failed");
        continue;
      }
    };

    let grant = corporation_service::refresh_grant(&corp, &token);

    let raw = match esi.corporation(corp_id).auth(&grant).assets().await {
      Ok(r) => r,
      Err(e) => {
        tracing::warn!("background sync: failed to fetch assets for corp {corp_id}: {e}");
        continue;
      }
    };

    let assets: Vec<CorporationAsset> = raw
      .into_iter()
      .map(|a| CorporationAsset {
        corporation_id: corp_id,
        is_blueprint_copy: a.is_blueprint_copy,
        is_singleton: a.is_singleton,
        item_id: a.item_id,
        location_flag: a.location_flag,
        location_id: a.location_id,
        location_type: a.location_type,
        quantity: a.quantity,
        type_id: a.type_id,
      })
      .collect();

    let keep_ids: Vec<i64> = assets.iter().map(|a| a.item_id).collect();
    let _ = db.assets().upsert_corporation_assets(corp_id, &assets).await;
    let _ = db.assets().delete_stale_corporation_assets(corp_id, &keep_ids).await;
    let _ = db
      .assets()
      .upsert_asset_sync_state("corporation", corp_id, Some(now), Some(now + 3600))
      .await;
  }
}
async fn propagate_skill_names(character: &mut Character, esi: &Client) {
  let resolved = character_service::inject_skill_names(character.skills().to_vec(), esi).await;
  *character.skills_mut() = resolved;
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
    .map(|(id, _, _)| id)
    .collect();

  let mut resolved: Vec<(i64, String, Option<i64>)> = Vec::new();
  for id in structure_ids {
    if cached.contains(&id) {
      continue;
    }
    if let Ok(info) = esi.universe().structure(id).auth(grant).detail().await {
      resolved.push((id, info.name, Some(info.solar_system_id)));
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

async fn step(tx: &mut Tx, label: String) {
  let _ = tx.send(Message::StepChanged(label)).await;
}
