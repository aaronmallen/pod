//! Character detail controller: parallel ESI data loading and view routing.

use std::collections::HashMap;

use pod_model::Character;
pub use pod_ui::views::character_detail::{LoadState, Message, State};
use pod_ui::{
  components::character_picker::{self, CharacterEntry, PickerSelection},
  views::character_detail::{ContactFilter, KilllogFilter, NotificationsFilter, Tab},
};

use crate::services::{Services, character as character_service};

/// Creates a new character detail state and fires ESI fetch tasks for enabled features.
pub fn new(
  character_id: i64,
  character: Character,
  all_characters: Vec<Character>,
  services: &Services,
) -> (State, iced::Task<Message>) {
  let features = services.config.features();
  let feat_clone_monitoring = *features.clone_monitoring();
  let feat_contacts = *features.contacts();
  let feat_combat_log = *features.combat_log();
  let feat_eve_notifications = *features.eve_notifications();
  let feat_standings = *features.standings();

  let picker = build_picker(character_id, all_characters);
  let first_enabled_tab = resolve_first_tab(
    feat_clone_monitoring,
    feat_contacts,
    feat_combat_log,
    feat_eve_notifications,
    feat_standings,
  );
  let state = build_initial_state(character.clone(), character_id, picker, first_enabled_tab, features);
  let task = build_fetch_tasks(
    &character,
    services,
    feat_clone_monitoring,
    feat_contacts,
    feat_combat_log,
    feat_eve_notifications,
    feat_standings,
  );
  (state, task)
}

/// Processes a character detail message, returning a follow-up task.
pub fn update(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::TabChanged(tab) => {
      tracing::info!(
        "character: tab selected — {tab:?}, character_id: {}",
        state.character_id
      );
      state.active_tab = tab;
      iced::Task::none()
    }
    Message::ContactsFilterChanged(f) => {
      tracing::info!("character: contacts filter changed — {f:?}");
      state.contact_filter = f;
      recompute_contact_filter(state);
      iced::Task::none()
    }
    Message::KilllogFilterChanged(f) => {
      tracing::info!("character: killlog filter changed — {f:?}");
      state.killlog_filter = f;
      recompute_killlog_filter(state);
      iced::Task::none()
    }
    Message::NotificationsFilterChanged(f) => {
      tracing::info!("character: notifications filter changed — {f:?}");
      state.notifications_filter = f;
      recompute_notifications_filter(state);
      iced::Task::none()
    }
    Message::NotificationRead(id) => handle_notification_read(state, id),
    Message::CharacterSwitched(_) | Message::NavigateToDetail(_) | Message::ReauthorizeCharacter(_) => {
      iced::Task::none()
    }
    Message::Picker(msg) => {
      state.picker.update(msg);
      iced::Task::none()
    }
    msg => dispatch_loaded(state, msg, services),
  }
}

fn build_picker(character_id: i64, all_characters: Vec<Character>) -> character_picker::Component {
  let picker_entries: Vec<CharacterEntry> = all_characters
    .into_iter()
    .map(|c| CharacterEntry {
      id: Some(*c.id()),
      name: c.name().clone(),
      corp_name: c.corp_name().clone(),
      tone: (*c.portrait_tone() as u16) % 360,
      portrait_handle: c.portrait_data().clone().map(iced::widget::image::Handle::from_bytes),
    })
    .collect();
  let mut picker = character_picker::Component::new()
    .entries(picker_entries)
    .show_all(false);
  picker.selected = PickerSelection::Character(character_id);
  picker
}

fn resolve_first_tab(
  feat_clone_monitoring: bool,
  feat_contacts: bool,
  feat_combat_log: bool,
  feat_eve_notifications: bool,
  feat_standings: bool,
) -> Tab {
  if feat_clone_monitoring {
    Tab::Clones
  } else if feat_contacts {
    Tab::Contacts
  } else if feat_combat_log {
    Tab::Killlog
  } else if feat_eve_notifications {
    Tab::Notifications
  } else if feat_standings {
    Tab::Standings
  } else {
    Tab::Clones
  }
}

fn build_initial_state(
  character: Character,
  character_id: i64,
  picker: character_picker::Component,
  first_enabled_tab: Tab,
  features: &crate::config::features::Settings,
) -> State {
  State {
    active_tab: first_enabled_tab,
    character,
    character_id,
    clones: LoadState::Loading,
    contact_filter: Default::default(),
    contact_labels: Vec::new(),
    contacts: LoadState::Loading,
    feat_clone_monitoring: *features.clone_monitoring(),
    feat_contacts: *features.contacts(),
    feat_combat_log: *features.combat_log(),
    feat_eve_notifications: *features.eve_notifications(),
    feat_location_tracking: *features.location_tracking(),
    feat_skill_monitoring: *features.skill_monitoring(),
    feat_standings: *features.standings(),
    feat_wallet: *features.wallet(),
    filtered_contacts: Vec::new(),
    filtered_killlog: Vec::new(),
    filtered_notifications: Vec::new(),
    implant_icons: HashMap::new(),
    killlog: LoadState::Loading,
    killlog_filter: Default::default(),
    notifications: LoadState::Loading,
    notifications_filter: Default::default(),
    picker,
    ship_icons: HashMap::new(),
    standings: LoadState::Loading,
    unread_notification_count: 0,
  }
}

#[allow(clippy::too_many_arguments)]
fn build_fetch_tasks(
  character: &Character,
  services: &Services,
  feat_clone_monitoring: bool,
  feat_contacts: bool,
  feat_combat_log: bool,
  feat_eve_notifications: bool,
  feat_standings: bool,
) -> iced::Task<Message> {
  let Some(esi) = services.esi_client.clone() else {
    return iced::Task::none();
  };
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  let mut tasks = Vec::new();
  if feat_clone_monitoring {
    tasks.push(clones_task(character.clone(), esi.clone(), db.clone()));
  }
  if feat_contacts {
    tasks.push(contacts_task(character.clone(), esi.clone(), db.clone()));
  }
  if feat_combat_log {
    tasks.push(killlog_task(character.clone(), esi.clone(), db.clone()));
  }
  if feat_eve_notifications {
    tasks.push(notifications_task(character.clone(), esi.clone(), db.clone()));
  }
  if feat_standings {
    tasks.push(standings_task(character.clone(), esi.clone(), db.clone()));
  }
  iced::Task::batch(tasks)
}

fn handle_notification_read(state: &mut State, id: i64) -> iced::Task<Message> {
  tracing::info!(
    "character: notification read — notification_id: {id}, character_id: {}",
    state.character_id
  );
  if let LoadState::Loaded(ref mut notifications) = state.notifications
    && let Some(n) = notifications.iter_mut().find(|n| n.notification_id == id)
  {
    n.is_read = true;
  }
  recompute_notifications_filter(state);
  iced::Task::none()
}

fn dispatch_loaded(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::ClonesLoaded(result) => handle_clones_loaded(state, result, services),
    Message::ImplantIconsLoaded(icons) => {
      insert_image_handles(&mut state.implant_icons, icons);
      iced::Task::none()
    }
    Message::KilllogLoaded(result) => handle_killlog_loaded(state, result, services),
    Message::ShipIconsLoaded(icons) => {
      insert_image_handles(&mut state.ship_icons, icons);
      iced::Task::none()
    }
    msg => dispatch_loaded_results(state, msg),
  }
}

fn dispatch_loaded_results(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::ContactsLoaded(result) => handle_contacts_loaded(state, result),
    Message::NotificationsLoaded(result) => handle_notifications_loaded(state, result),
    Message::StandingsLoaded(result) => handle_standings_loaded(state, result),
    _ => iced::Task::none(),
  }
}

fn handle_contacts_loaded(
  state: &mut State,
  result: Result<(Vec<pod_model::CharacterContact>, Vec<pod_model::CharacterContactLabel>), String>,
) -> iced::Task<Message> {
  match result {
    Ok((contacts, labels)) => {
      tracing::debug!(
        "character: {} contacts loaded — character_id: {}",
        contacts.len(),
        state.character_id
      );
      state.contact_labels = labels;
      state.contacts = LoadState::Loaded(contacts);
      recompute_contact_filter(state);
      iced::Task::none()
    }
    Err(e) => {
      tracing::warn!(
        "character: contacts load failed — character_id: {}, error: {e}",
        state.character_id
      );
      state.contacts = LoadState::Error(e);
      iced::Task::none()
    }
  }
}

fn handle_notifications_loaded(
  state: &mut State,
  result: Result<Vec<pod_model::CharacterNotification>, String>,
) -> iced::Task<Message> {
  match result {
    Ok(notifications) => {
      tracing::debug!(
        "character: {} notifications loaded — character_id: {}",
        notifications.len(),
        state.character_id
      );
      state.notifications = LoadState::Loaded(notifications);
      recompute_notifications_filter(state);
      iced::Task::none()
    }
    Err(e) => {
      tracing::warn!(
        "character: notifications load failed — character_id: {}, error: {e}",
        state.character_id
      );
      state.notifications = LoadState::Error(e);
      iced::Task::none()
    }
  }
}

fn handle_standings_loaded(
  state: &mut State,
  result: Result<Vec<pod_model::CharacterStanding>, String>,
) -> iced::Task<Message> {
  match result {
    Ok(standings) => {
      tracing::debug!(
        "character: {} standings loaded — character_id: {}",
        standings.len(),
        state.character_id
      );
      state.standings = LoadState::Loaded(standings);
      iced::Task::none()
    }
    Err(e) => {
      tracing::warn!(
        "character: standings load failed — character_id: {}, error: {e}",
        state.character_id
      );
      state.standings = LoadState::Error(e);
      iced::Task::none()
    }
  }
}

fn insert_image_handles(map: &mut HashMap<i32, iced::widget::image::Handle>, icons: Vec<(i32, Vec<u8>)>) {
  for (type_id, bytes) in icons {
    map.insert(type_id, iced::widget::image::Handle::from_bytes(bytes));
  }
}

fn handle_clones_loaded(
  state: &mut State,
  result: Result<Vec<pod_model::CharacterClone>, String>,
  services: &Services,
) -> iced::Task<Message> {
  match result {
    Ok(clones) => {
      tracing::debug!(
        "character: {} clones loaded — character_id: {}",
        clones.len(),
        state.character_id
      );
      let type_ids: Vec<i32> = clones
        .iter()
        .flat_map(|c| c.implants.iter().map(|i| i.type_id))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .filter(|id| !state.implant_icons.contains_key(id))
        .collect();
      state.clones = LoadState::Loaded(clones);
      if !type_ids.is_empty()
        && let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone())
      {
        return iced::Task::perform(
          async move { load_type_icons(type_ids, esi, db).await },
          Message::ImplantIconsLoaded,
        );
      }
      iced::Task::none()
    }
    Err(e) => {
      tracing::warn!(
        "character: clones load failed — character_id: {}, error: {e}",
        state.character_id
      );
      state.clones = LoadState::Error(e);
      iced::Task::none()
    }
  }
}

fn handle_killlog_loaded(
  state: &mut State,
  result: Result<Vec<pod_model::CharacterKillEntry>, String>,
  services: &Services,
) -> iced::Task<Message> {
  match result {
    Ok(entries) => {
      tracing::debug!(
        "character: {} killlog entries loaded — character_id: {}",
        entries.len(),
        state.character_id
      );
      let type_ids: Vec<i32> = entries
        .iter()
        .map(|e| e.ship_type_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .filter(|&id| id != 0 && !state.ship_icons.contains_key(&id))
        .collect();
      state.killlog = LoadState::Loaded(entries);
      recompute_killlog_filter(state);
      if !type_ids.is_empty()
        && let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone())
      {
        return iced::Task::perform(
          async move { load_type_icons(type_ids, esi, db).await },
          Message::ShipIconsLoaded,
        );
      }
      iced::Task::none()
    }
    Err(e) => {
      tracing::warn!(
        "character: killlog load failed — character_id: {}, error: {e}",
        state.character_id
      );
      state.killlog = LoadState::Error(e);
      iced::Task::none()
    }
  }
}

fn recompute_contact_filter(state: &mut State) {
  let LoadState::Loaded(contacts) = &state.contacts else {
    state.filtered_contacts = Vec::new();
    return;
  };
  state.filtered_contacts = contacts
    .iter()
    .filter(|c| match &state.contact_filter {
      ContactFilter::All => true,
      ContactFilter::Character => c.contact_type == "character",
      ContactFilter::Corp => c.contact_type == "corporation",
      ContactFilter::Alliance => c.contact_type == "alliance",
    })
    .cloned()
    .collect();
}

fn recompute_killlog_filter(state: &mut State) {
  let LoadState::Loaded(entries) = &state.killlog else {
    state.filtered_killlog = Vec::new();
    return;
  };
  state.filtered_killlog = entries
    .iter()
    .filter(|e| match &state.killlog_filter {
      KilllogFilter::All => true,
      KilllogFilter::Kill => e.is_kill,
      KilllogFilter::Loss => !e.is_kill,
    })
    .cloned()
    .collect();
}

fn recompute_notifications_filter(state: &mut State) {
  let LoadState::Loaded(notifications) = &state.notifications else {
    state.filtered_notifications = Vec::new();
    state.unread_notification_count = 0;
    return;
  };
  state.unread_notification_count = notifications.iter().filter(|n| !n.is_read).count();
  state.filtered_notifications = notifications
    .iter()
    .filter(|n| match &state.notifications_filter {
      NotificationsFilter::All => true,
      NotificationsFilter::Unread => !n.is_read,
      NotificationsFilter::Combat => n.category == "combat",
      NotificationsFilter::Corp => n.category == "corp",
      NotificationsFilter::Structure => n.category == "structure",
      NotificationsFilter::War => n.category == "war",
    })
    .cloned()
    .collect();
}

fn clones_task(character: Character, esi: pod_esi::Client, db: pod_db::Repo) -> iced::Task<Message> {
  iced::Task::perform(load_clones(character, esi, db), Message::ClonesLoaded)
}

fn contacts_task(character: Character, esi: pod_esi::Client, db: pod_db::Repo) -> iced::Task<Message> {
  iced::Task::perform(load_contacts(character, esi, db), Message::ContactsLoaded)
}

fn killlog_task(character: Character, esi: pod_esi::Client, db: pod_db::Repo) -> iced::Task<Message> {
  iced::Task::perform(load_killlog(character, esi, db), Message::KilllogLoaded)
}

fn notifications_task(character: Character, esi: pod_esi::Client, db: pod_db::Repo) -> iced::Task<Message> {
  iced::Task::perform(load_notifications(character, esi, db), Message::NotificationsLoaded)
}

fn standings_task(character: Character, esi: pod_esi::Client, db: pod_db::Repo) -> iced::Task<Message> {
  iced::Task::perform(load_standings(character, esi, db), Message::StandingsLoaded)
}

async fn load_clones(
  character: Character,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Result<Vec<pod_model::CharacterClone>, String> {
  let Some((token, _)) = character_service::ensure_valid_token(&character, &esi, &db).await else {
    return Err("token refresh failed".into());
  };
  let grant = character_service::refresh_grant(&character, &token);
  let char_client = esi.character(&grant);

  let (clones_res, implants_res) = tokio::join!(char_client.clones(), char_client.implants());
  let clones_data = clones_res.map_err(|e| e.to_string())?;
  let active_implant_ids = implants_res.unwrap_or_default();

  let all_type_ids = collect_clone_type_ids(&active_implant_ids, &clones_data.jump_clones);
  let (name_map, slot_map) = load_implant_type_maps(&all_type_ids, &db).await;

  let home_loc_id = clones_data.home_location.as_ref().and_then(|l| l.location_id);
  let jump_loc_ids: Vec<i64> = clones_data.jump_clones.iter().map(|jc| jc.location_id).collect();
  let all_loc_ids: Vec<i64> = home_loc_id.into_iter().chain(jump_loc_ids.iter().copied()).collect();
  let (db_station_map, esi_name_map) = resolve_location_names(&all_loc_ids, &db, &esi).await;

  Ok(assemble_clone_list(
    clones_data,
    &active_implant_ids,
    home_loc_id,
    &name_map,
    &slot_map,
    &db_station_map,
    &esi_name_map,
  ))
}

async fn load_implant_type_maps(
  all_type_ids: &[i32],
  db: &pod_db::Repo,
) -> (HashMap<i32, String>, HashMap<i32, usize>) {
  let type_rows = db
    .universe()
    .item_types()
    .find_by_ids(all_type_ids)
    .await
    .unwrap_or_default();
  let name_map: HashMap<i32, String> = type_rows.iter().map(|t| (t.id, t.name.clone())).collect();
  let slot_map: HashMap<i32, usize> = type_rows
    .into_iter()
    .filter_map(|t| {
      t.dogma_attributes
        .0
        .iter()
        .find(|a| a.attribute_id == 331)
        .map(|a| (t.id, a.value as usize))
    })
    .collect();
  (name_map, slot_map)
}

#[allow(clippy::too_many_arguments)]
fn assemble_clone_list(
  clones_data: pod_esi::models::character::Clones,
  active_implant_ids: &[i32],
  home_loc_id: Option<i64>,
  name_map: &HashMap<i32, String>,
  slot_map: &HashMap<i32, usize>,
  db_station_map: &HashMap<i64, String>,
  esi_name_map: &HashMap<i64, String>,
) -> Vec<pod_model::CharacterClone> {
  let mut result = Vec::with_capacity(1 + clones_data.jump_clones.len());
  let home_station_name = home_loc_id
    .map(|id| resolve_location(id, db_station_map, esi_name_map))
    .unwrap_or_default();
  result.push(pod_model::CharacterClone {
    clone_id: 0,
    implants: build_implants(active_implant_ids, name_map, slot_map),
    is_active: true,
    jump_ready_at: clones_data.last_clone_jump_date.clone(),
    name: None,
    station_name: home_station_name,
  });
  for jc in clones_data.jump_clones {
    let station_name = resolve_location(jc.location_id, db_station_map, esi_name_map);
    result.push(pod_model::CharacterClone {
      clone_id: jc.clone_id.unwrap_or(0),
      implants: build_implants(&jc.implants, name_map, slot_map),
      is_active: false,
      jump_ready_at: None,
      name: jc.name,
      station_name,
    });
  }
  result
}

fn collect_clone_type_ids(
  active_implant_ids: &[i32],
  jump_clones: &[pod_esi::models::character::JumpClone],
) -> Vec<i32> {
  let mut ids = active_implant_ids.to_vec();
  for jc in jump_clones {
    ids.extend_from_slice(&jc.implants);
  }
  ids.sort_unstable();
  ids.dedup();
  ids
}

fn build_implants(
  ids: &[i32],
  name_map: &HashMap<i32, String>,
  slot_map: &HashMap<i32, usize>,
) -> Vec<pod_model::CharacterImplant> {
  ids
    .iter()
    .enumerate()
    .map(|(i, &type_id)| pod_model::CharacterImplant {
      name: name_map.get(&type_id).cloned().unwrap_or_else(|| type_id.to_string()),
      slot: slot_map.get(&type_id).copied().unwrap_or(i + 1),
      type_id,
    })
    .collect()
}

fn resolve_location(loc_id: i64, db_station_map: &HashMap<i64, String>, esi_name_map: &HashMap<i64, String>) -> String {
  db_station_map
    .get(&loc_id)
    .or_else(|| esi_name_map.get(&loc_id))
    .cloned()
    .unwrap_or_else(|| loc_id.to_string())
}

async fn resolve_location_names(
  all_loc_ids: &[i64],
  db: &pod_db::Repo,
  esi: &pod_esi::Client,
) -> (HashMap<i64, String>, HashMap<i64, String>) {
  let station_ids: Vec<i32> = all_loc_ids
    .iter()
    .filter(|&&id| id < 100_000_000i64)
    .filter_map(|&id| i32::try_from(id).ok())
    .collect();
  let db_station_map: HashMap<i64, String> = db
    .universe()
    .stations()
    .find_by_ids(&station_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|s| (*s.id() as i64, s.name().clone()))
    .collect();
  let unresolved_ids: Vec<i64> = all_loc_ids
    .iter()
    .copied()
    .filter(|id| !db_station_map.contains_key(id))
    .collect();
  let esi_name_map: HashMap<i64, String> = if unresolved_ids.is_empty() {
    HashMap::new()
  } else {
    esi
      .universe()
      .names(&unresolved_ids)
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|n| (n.id, n.name))
      .collect()
  };
  (db_station_map, esi_name_map)
}

async fn load_contacts(
  character: Character,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Result<(Vec<pod_model::CharacterContact>, Vec<pod_model::CharacterContactLabel>), String> {
  let Some((token, _)) = character_service::ensure_valid_token(&character, &esi, &db).await else {
    return Err("token refresh failed".into());
  };
  let grant = character_service::refresh_grant(&character, &token);
  let char_client = esi.character(&grant);

  let (contacts_res, labels_res) = tokio::join!(char_client.contacts(), char_client.contact_labels());
  let esi_contacts = contacts_res.map_err(|e| e.to_string())?;
  let esi_labels = labels_res.unwrap_or_default();

  let label_map: HashMap<i64, String> = esi_labels.iter().map(|l| (l.label_id, l.label_name.clone())).collect();
  let labels: Vec<pod_model::CharacterContactLabel> = esi_labels
    .into_iter()
    .map(|l| pod_model::CharacterContactLabel {
      label_id: l.label_id,
      name: l.label_name,
    })
    .collect();

  let contact_ids: Vec<i64> = esi_contacts.iter().map(|c| c.contact_id).collect();
  let contact_name_map = resolve_contact_names(&contact_ids, &esi).await;

  let contacts = build_contacts(esi_contacts, &label_map, &contact_name_map);
  Ok((contacts, labels))
}

async fn resolve_contact_names(contact_ids: &[i64], esi: &pod_esi::Client) -> HashMap<i64, String> {
  if contact_ids.is_empty() {
    return HashMap::new();
  }
  esi
    .universe()
    .names(contact_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|n| (n.id, n.name))
    .collect()
}

fn build_contacts(
  esi_contacts: Vec<pod_esi::models::character::CharacterContact>,
  label_map: &HashMap<i64, String>,
  contact_name_map: &HashMap<i64, String>,
) -> Vec<pod_model::CharacterContact> {
  esi_contacts
    .into_iter()
    .map(|c| {
      let label_names = c
        .label_ids
        .iter()
        .flatten()
        .filter_map(|id| label_map.get(id).cloned())
        .collect();
      let name = contact_name_map
        .get(&c.contact_id)
        .cloned()
        .unwrap_or_else(|| c.contact_id.to_string());
      pod_model::CharacterContact {
        contact_id: c.contact_id,
        contact_type: c.contact_type,
        is_blocked: c.is_blocked.unwrap_or(false),
        is_watched: c.is_watched.unwrap_or(false),
        label_names,
        name,
        standing: c.standing,
      }
    })
    .collect()
}

async fn load_killlog(
  character: Character,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Result<Vec<pod_model::CharacterKillEntry>, String> {
  let Some((token, _)) = character_service::ensure_valid_token(&character, &esi, &db).await else {
    return Err("token refresh failed".into());
  };
  let grant = character_service::refresh_grant(&character, &token);
  let char_client = esi.character(&grant);

  let refs = char_client.killmails().await.map_err(|e| e.to_string())?;
  let mut raw_details = Vec::with_capacity(refs.len());
  for r in &refs {
    raw_details.push(esi.killmail(r.killmail_id, &r.killmail_hash).detail().await);
  }

  let char_id = *character.id();
  let system_ids = collect_system_ids(&raw_details);
  let system_rows = db
    .universe()
    .solar_systems()
    .find_by_ids(&system_ids)
    .await
    .unwrap_or_default();
  let system_map: HashMap<i32, String> = system_rows.iter().map(|s| (s.id, s.name.clone())).collect();
  let system_sec_map: HashMap<i32, f64> = system_rows.into_iter().map(|s| (s.id, s.security_status)).collect();

  let ship_type_ids = collect_ship_type_ids(&raw_details);
  let ship_map: HashMap<i32, String> = db
    .universe()
    .item_types()
    .find_by_ids(&ship_type_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|t| (t.id, t.name))
    .collect();

  let entity_ids = collect_killlog_entity_ids(&raw_details);
  let killmail_ids: Vec<i64> = raw_details.iter().flatten().map(|k| k.killmail_id).collect();

  let (entity_name_map, zkill_value_map) = tokio::join!(
    resolve_killlog_entity_names(&entity_ids, &esi),
    fetch_zkill_values(char_id, &killmail_ids),
  );

  let mut entries = Vec::new();
  for detail in raw_details.into_iter().flatten() {
    entries.push(build_kill_entry(
      detail,
      char_id,
      &system_map,
      &system_sec_map,
      &ship_map,
      &entity_name_map,
      &zkill_value_map,
    ));
  }
  Ok(entries)
}

fn collect_system_ids(raw_details: &[Result<pod_esi::models::killmail::Killmail, pod_esi::Error>]) -> Vec<i32> {
  raw_details
    .iter()
    .filter_map(|r| r.as_ref().ok())
    .map(|k| k.solar_system_id as i32)
    .collect::<std::collections::HashSet<_>>()
    .into_iter()
    .collect()
}

fn collect_ship_type_ids(raw_details: &[Result<pod_esi::models::killmail::Killmail, pod_esi::Error>]) -> Vec<i32> {
  raw_details
    .iter()
    .filter_map(|r| r.as_ref().ok())
    .filter_map(|k| {
      k.victim
        .get("ship_type_id")
        .and_then(|v| v.as_i64())
        .map(|id| id as i32)
    })
    .collect::<std::collections::HashSet<_>>()
    .into_iter()
    .collect()
}

fn collect_killlog_entity_ids(raw_details: &[Result<pod_esi::models::killmail::Killmail, pod_esi::Error>]) -> Vec<i64> {
  let mut ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
  for detail in raw_details.iter().flatten() {
    let victim_char_id = detail.victim.get("character_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let victim_corp_id = detail
      .victim
      .get("corporation_id")
      .and_then(|v| v.as_i64())
      .unwrap_or(0);
    if victim_char_id != 0 {
      ids.insert(victim_char_id);
    }
    if victim_corp_id != 0 {
      ids.insert(victim_corp_id);
    }
  }
  ids.into_iter().collect()
}

async fn resolve_killlog_entity_names(entity_ids: &[i64], esi: &pod_esi::Client) -> HashMap<i64, String> {
  if entity_ids.is_empty() {
    return HashMap::new();
  }
  esi
    .universe()
    .names(entity_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|n| (n.id, n.name))
    .collect()
}

fn extract_final_blow(attackers: &[serde_json::Value], char_id: i64) -> bool {
  attackers.iter().any(|a| {
    a.get("final_blow").and_then(|v| v.as_bool()).unwrap_or(false)
      && a.get("character_id").and_then(|v| v.as_i64()).unwrap_or(0) == char_id
  })
}

fn get_json_i64(value: &serde_json::Value, key: &str) -> i64 {
  value.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
fn build_kill_entry(
  detail: pod_esi::models::killmail::Killmail,
  char_id: i64,
  system_map: &HashMap<i32, String>,
  system_sec_map: &HashMap<i32, f64>,
  ship_map: &HashMap<i32, String>,
  entity_name_map: &HashMap<i64, String>,
  zkill_value_map: &HashMap<i64, f64>,
) -> pod_model::CharacterKillEntry {
  let victim_char_id = get_json_i64(&detail.victim, "character_id");
  let is_kill = victim_char_id != char_id;
  let ship_type_id = get_json_i64(&detail.victim, "ship_type_id") as i32;
  let attacker_count = detail.attackers.len() as u32;
  let final_blow = extract_final_blow(&detail.attackers, char_id);
  let victim_corp_id = get_json_i64(&detail.victim, "corporation_id");
  let victim_name = entity_name_map.get(&victim_char_id).cloned().unwrap_or_default();
  let victim_corp = entity_name_map.get(&victim_corp_id).cloned().unwrap_or_default();
  let total_value = zkill_value_map.get(&detail.killmail_id).copied().unwrap_or(0.0);
  let sys_id = detail.solar_system_id as i32;
  pod_model::CharacterKillEntry {
    attacker_count,
    final_blow,
    is_kill,
    killmail_id: detail.killmail_id,
    ship_name: ship_map
      .get(&ship_type_id)
      .cloned()
      .unwrap_or_else(|| ship_type_id.to_string()),
    ship_type_id,
    solar_system_name: system_map
      .get(&sys_id)
      .cloned()
      .unwrap_or_else(|| detail.solar_system_id.to_string()),
    solar_system_security: system_sec_map.get(&sys_id).copied().unwrap_or(0.0),
    timestamp: detail.killmail_time,
    total_value,
    victim_corp,
    victim_name,
  }
}

async fn fetch_zkill_values(char_id: i64, killmail_ids: &[i64]) -> HashMap<i64, f64> {
  if killmail_ids.is_empty() {
    return HashMap::new();
  }
  let url = format!("https://zkillboard.com/api/characterID/{char_id}/");
  let Ok(resp) = reqwest::Client::new()
    .get(&url)
    .header("User-Agent", "pod-app/1.0 (github.com/aaronmallen/pod)")
    .header("Accept", "application/json")
    .send()
    .await
  else {
    return HashMap::new();
  };
  let Ok(json) = resp.json::<Vec<serde_json::Value>>().await else {
    return HashMap::new();
  };
  json
    .into_iter()
    .filter_map(|v| {
      let id = v.get("killmail_id")?.as_i64()?;
      let value = v.get("zkb")?.get("totalValue")?.as_f64()?;
      Some((id, value))
    })
    .collect()
}

async fn load_notifications(
  character: Character,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Result<Vec<pod_model::CharacterNotification>, String> {
  let Some((token, _)) = character_service::ensure_valid_token(&character, &esi, &db).await else {
    return Err("token refresh failed".into());
  };
  let grant = character_service::refresh_grant(&character, &token);
  let char_client = esi.character(&grant);

  let esi_notifications = char_client.notifications().await.map_err(|e| e.to_string())?;
  let notifications = esi_notifications.into_iter().map(map_notification).collect();
  Ok(notifications)
}

fn map_notification(n: pod_esi::models::character::CharacterNotification) -> pod_model::CharacterNotification {
  use pod_model::categorize_notif;
  let category = categorize_notif(&n.r#type);
  let cat_str = notification_category_label_a(&category).unwrap_or_else(|| notification_category_label_b(&category));
  pod_model::CharacterNotification {
    category: cat_str.to_string(),
    is_read: n.is_read.unwrap_or(false),
    notification_id: n.notification_id,
    sender_id: n.sender_id,
    sender_type: n.sender_type,
    text: n.text,
    timestamp: n.timestamp,
    type_: n.r#type,
  }
}

fn notification_category_label_a(cat: &pod_model::NotificationCategory) -> Option<&'static str> {
  use pod_model::NotificationCategory;
  match cat {
    NotificationCategory::Alliance => Some("alliance"),
    NotificationCategory::Clone => Some("clone"),
    NotificationCategory::Combat => Some("combat"),
    NotificationCategory::Contact => Some("contact"),
    NotificationCategory::Contract => Some("contract"),
    cat => notification_category_label_a_ext(cat),
  }
}

fn notification_category_label_a_ext(cat: &pod_model::NotificationCategory) -> Option<&'static str> {
  use pod_model::NotificationCategory;
  match cat {
    NotificationCategory::Corp => Some("corp"),
    NotificationCategory::Fw => Some("fw"),
    NotificationCategory::Incursion => Some("incursion"),
    NotificationCategory::Industry => Some("industry"),
    _ => None,
  }
}

fn notification_category_label_b(cat: &pod_model::NotificationCategory) -> &'static str {
  use pod_model::NotificationCategory;
  match cat {
    NotificationCategory::Insurance => "insurance",
    NotificationCategory::Market => "market",
    NotificationCategory::Mission => "mission",
    NotificationCategory::Reward => "reward",
    NotificationCategory::Standing => "standing",
    NotificationCategory::Structure => "structure",
    NotificationCategory::System => "system",
    NotificationCategory::War => "war",
    _ => "other",
  }
}

async fn load_standings(
  character: Character,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Result<Vec<pod_model::CharacterStanding>, String> {
  let Some((token, _)) = character_service::ensure_valid_token(&character, &esi, &db).await else {
    return Err("token refresh failed".into());
  };
  let grant = character_service::refresh_grant(&character, &token);
  let char_client = esi.character(&grant);

  let esi_standings = char_client.standings().await.map_err(|e| e.to_string())?;
  let standing_ids: Vec<i64> = esi_standings.iter().map(|s| s.from_id).collect();
  let standing_name_map: HashMap<i64, String> = if standing_ids.is_empty() {
    HashMap::new()
  } else {
    esi
      .universe()
      .names(&standing_ids)
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|n| (n.id, n.name))
      .collect()
  };

  let standings = esi_standings
    .into_iter()
    .map(|s| pod_model::CharacterStanding {
      from_id: s.from_id,
      from_name: standing_name_map
        .get(&s.from_id)
        .cloned()
        .unwrap_or_else(|| s.from_id.to_string()),
      from_type: s.from_type,
      standing: s.standing,
    })
    .collect();
  Ok(standings)
}

async fn load_type_icons(type_ids: Vec<i32>, esi: pod_esi::Client, db: pod_db::Repo) -> Vec<(i32, Vec<u8>)> {
  let cached: HashMap<i32, Vec<u8>> = db
    .universe()
    .type_icons()
    .find_by_ids(&type_ids, "icon")
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

  let mut result: Vec<(i32, Vec<u8>)> = cached.into_iter().collect();
  let missing: Vec<i32> = type_ids
    .into_iter()
    .filter(|id| !result.iter().any(|(k, _)| k == id))
    .collect();

  for type_id in missing {
    if let Ok(bytes) = esi.images().type_icon(type_id as i64, 64).await {
      // write-through icon cache; failure is safe to ignore
      let _ = db.universe().type_icons().upsert(type_id, "icon", bytes.clone()).await;
      result.push((type_id, bytes));
    }
  }
  result
}
