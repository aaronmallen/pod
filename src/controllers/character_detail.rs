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

  let first_enabled_tab = if feat_clone_monitoring {
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
  };

  let state = State {
    active_tab: first_enabled_tab,
    character: character.clone(),
    character_id,
    clones: LoadState::Loading,
    contact_filter: Default::default(),
    contact_labels: Vec::new(),
    contacts: LoadState::Loading,
    feat_clone_monitoring,
    feat_contacts,
    feat_combat_log,
    feat_eve_notifications,
    feat_location_tracking: *features.location_tracking(),
    feat_skill_monitoring: *features.skill_monitoring(),
    feat_standings,
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
  };

  let task = if let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone()) {
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
  } else {
    iced::Task::none()
  };

  (state, task)
}

/// Processes a character detail message, returning a follow-up task.
pub fn update(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::TabChanged(tab) => {
      state.active_tab = tab;
      iced::Task::none()
    }
    Message::ContactsFilterChanged(f) => {
      state.contact_filter = f;
      recompute_contact_filter(state);
      iced::Task::none()
    }
    Message::KilllogFilterChanged(f) => {
      state.killlog_filter = f;
      recompute_killlog_filter(state);
      iced::Task::none()
    }
    Message::NotificationsFilterChanged(f) => {
      state.notifications_filter = f;
      recompute_notifications_filter(state);
      iced::Task::none()
    }
    Message::NotificationRead(id) => {
      if let LoadState::Loaded(ref mut notifications) = state.notifications
        && let Some(n) = notifications.iter_mut().find(|n| n.notification_id == id)
      {
        n.is_read = true;
      }
      recompute_notifications_filter(state);
      iced::Task::none()
    }
    Message::CharacterSwitched(_) | Message::NavigateToDetail(_) => iced::Task::none(),
    Message::Picker(msg) => {
      state.picker.update(msg);
      iced::Task::none()
    }
    Message::ClonesLoaded(Ok(clones)) => {
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
    Message::ClonesLoaded(Err(e)) => {
      state.clones = LoadState::Error(e);
      iced::Task::none()
    }
    Message::ImplantIconsLoaded(icons) => {
      for (type_id, bytes) in icons {
        state
          .implant_icons
          .insert(type_id, iced::widget::image::Handle::from_bytes(bytes));
      }
      iced::Task::none()
    }
    Message::ContactsLoaded(Ok((contacts, labels))) => {
      state.contact_labels = labels;
      state.contacts = LoadState::Loaded(contacts);
      recompute_contact_filter(state);
      iced::Task::none()
    }
    Message::ContactsLoaded(Err(e)) => {
      state.contacts = LoadState::Error(e);
      iced::Task::none()
    }
    Message::KilllogLoaded(Ok(entries)) => {
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
    Message::KilllogLoaded(Err(e)) => {
      state.killlog = LoadState::Error(e);
      iced::Task::none()
    }
    Message::ShipIconsLoaded(icons) => {
      for (type_id, bytes) in icons {
        state
          .ship_icons
          .insert(type_id, iced::widget::image::Handle::from_bytes(bytes));
      }
      iced::Task::none()
    }
    Message::NotificationsLoaded(Ok(notifications)) => {
      state.notifications = LoadState::Loaded(notifications);
      recompute_notifications_filter(state);
      iced::Task::none()
    }
    Message::NotificationsLoaded(Err(e)) => {
      state.notifications = LoadState::Error(e);
      iced::Task::none()
    }
    Message::StandingsLoaded(Ok(standings)) => {
      state.standings = LoadState::Loaded(standings);
      iced::Task::none()
    }
    Message::StandingsLoaded(Err(e)) => {
      state.standings = LoadState::Error(e);
      iced::Task::none()
    }
    Message::ReauthorizeCharacter(_) => iced::Task::none(),
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
  let Some(token) = character_service::ensure_valid_token(&character, &esi, &db).await else {
    return Err("token refresh failed".into());
  };
  let grant = character_service::refresh_grant(&character, &token);
  let char_client = esi.character(&grant);

  let (clones_res, implants_res) = tokio::join!(char_client.clones(), char_client.implants());

  let clones_data = clones_res.map_err(|e| e.to_string())?;
  let active_implant_ids = implants_res.unwrap_or_default();

  let all_type_ids: Vec<i32> = {
    let mut ids = active_implant_ids.clone();
    for jc in &clones_data.jump_clones {
      ids.extend_from_slice(&jc.implants);
    }
    ids.sort_unstable();
    ids.dedup();
    ids
  };

  let type_rows = db
    .universe()
    .item_types()
    .find_by_ids(&all_type_ids)
    .await
    .unwrap_or_default();

  let name_map: std::collections::HashMap<i32, String> = type_rows.iter().map(|t| (t.id, t.name.clone())).collect();

  // Dogma attribute 331 = implantSlotModifier — gives the physical slot (1–10).
  let slot_map: std::collections::HashMap<i32, usize> = type_rows
    .into_iter()
    .filter_map(|t| {
      t.dogma_attributes
        .0
        .iter()
        .find(|a| a.attribute_id == 331)
        .map(|a| (t.id, a.value as usize))
    })
    .collect();

  let build_implants = |ids: &[i32]| -> Vec<pod_model::CharacterImplant> {
    ids
      .iter()
      .enumerate()
      .map(|(i, &type_id)| pod_model::CharacterImplant {
        name: name_map.get(&type_id).cloned().unwrap_or_else(|| type_id.to_string()),
        slot: slot_map.get(&type_id).copied().unwrap_or(i + 1),
        type_id,
      })
      .collect()
  };

  let home_loc_id = clones_data.home_location.as_ref().and_then(|l| l.location_id);
  let jump_loc_ids: Vec<i64> = clones_data.jump_clones.iter().map(|jc| jc.location_id).collect();

  let all_loc_ids: Vec<i64> = home_loc_id.into_iter().chain(jump_loc_ids.iter().copied()).collect();

  let station_ids: Vec<i32> = all_loc_ids
    .iter()
    .filter(|&&id| id < 100_000_000i64)
    .filter_map(|&id| i32::try_from(id).ok())
    .collect();

  let db_station_map: std::collections::HashMap<i64, String> = db
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

  let esi_name_map: std::collections::HashMap<i64, String> = if unresolved_ids.is_empty() {
    std::collections::HashMap::new()
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

  let resolve_location = |loc_id: i64| -> String {
    db_station_map
      .get(&loc_id)
      .or_else(|| esi_name_map.get(&loc_id))
      .cloned()
      .unwrap_or_else(|| loc_id.to_string())
  };

  let mut result = Vec::with_capacity(1 + clones_data.jump_clones.len());

  let home_station_name = home_loc_id.map(resolve_location).unwrap_or_default();

  result.push(pod_model::CharacterClone {
    clone_id: 0,
    implants: build_implants(&active_implant_ids),
    is_active: true,
    jump_ready_at: clones_data.last_clone_jump_date.clone(),
    name: None,
    station_name: home_station_name,
  });

  for jc in clones_data.jump_clones {
    let station_name = resolve_location(jc.location_id);
    result.push(pod_model::CharacterClone {
      clone_id: jc.clone_id.unwrap_or(0),
      implants: build_implants(&jc.implants),
      is_active: false,
      jump_ready_at: None,
      name: jc.name,
      station_name,
    });
  }

  Ok(result)
}

async fn load_contacts(
  character: Character,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Result<(Vec<pod_model::CharacterContact>, Vec<pod_model::CharacterContactLabel>), String> {
  let Some(token) = character_service::ensure_valid_token(&character, &esi, &db).await else {
    return Err("token refresh failed".into());
  };
  let grant = character_service::refresh_grant(&character, &token);
  let char_client = esi.character(&grant);

  let (contacts_res, labels_res) = tokio::join!(char_client.contacts(), char_client.contact_labels());

  let esi_contacts = contacts_res.map_err(|e| e.to_string())?;
  let esi_labels = labels_res.unwrap_or_default();

  let label_map: std::collections::HashMap<i64, String> =
    esi_labels.iter().map(|l| (l.label_id, l.label_name.clone())).collect();

  let labels: Vec<pod_model::CharacterContactLabel> = esi_labels
    .into_iter()
    .map(|l| pod_model::CharacterContactLabel {
      label_id: l.label_id,
      name: l.label_name,
    })
    .collect();

  let contact_ids: Vec<i64> = esi_contacts.iter().map(|c| c.contact_id).collect();

  let contact_name_map: std::collections::HashMap<i64, String> = if contact_ids.is_empty() {
    std::collections::HashMap::new()
  } else {
    esi
      .universe()
      .names(&contact_ids)
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|n| (n.id, n.name))
      .collect()
  };

  let contacts: Vec<pod_model::CharacterContact> = esi_contacts
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
    .collect();

  Ok((contacts, labels))
}

async fn load_killlog(
  character: Character,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Result<Vec<pod_model::CharacterKillEntry>, String> {
  let Some(token) = character_service::ensure_valid_token(&character, &esi, &db).await else {
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

  let system_ids: Vec<i32> = raw_details
    .iter()
    .filter_map(|r| r.as_ref().ok())
    .map(|k| k.solar_system_id as i32)
    .collect::<std::collections::HashSet<_>>()
    .into_iter()
    .collect();

  let system_rows = db
    .universe()
    .solar_systems()
    .find_by_ids(&system_ids)
    .await
    .unwrap_or_default();
  let system_map: std::collections::HashMap<i32, String> = system_rows.iter().map(|s| (s.id, s.name.clone())).collect();
  let system_sec_map: std::collections::HashMap<i32, f64> =
    system_rows.into_iter().map(|s| (s.id, s.security_status)).collect();

  let ship_type_ids: Vec<i32> = raw_details
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
    .collect();

  let ship_map: std::collections::HashMap<i32, String> = db
    .universe()
    .item_types()
    .find_by_ids(&ship_type_ids)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|t| (t.id, t.name))
    .collect();

  let mut entity_ids_to_resolve: std::collections::HashSet<i64> = std::collections::HashSet::new();
  for detail in raw_details.iter().flatten() {
    let victim_char_id = detail.victim.get("character_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let victim_corp_id = detail
      .victim
      .get("corporation_id")
      .and_then(|v| v.as_i64())
      .unwrap_or(0);
    if victim_char_id != 0 {
      entity_ids_to_resolve.insert(victim_char_id);
    }
    if victim_corp_id != 0 {
      entity_ids_to_resolve.insert(victim_corp_id);
    }
  }

  let entity_ids: Vec<i64> = entity_ids_to_resolve.into_iter().collect();
  let killmail_ids: Vec<i64> = raw_details.iter().flatten().map(|k| k.killmail_id).collect();

  let (entity_name_map, zkill_value_map) = tokio::join!(
    async {
      if entity_ids.is_empty() {
        HashMap::new()
      } else {
        esi
          .universe()
          .names(&entity_ids)
          .await
          .unwrap_or_default()
          .into_iter()
          .map(|n| (n.id, n.name))
          .collect()
      }
    },
    fetch_zkill_values(char_id, &killmail_ids),
  );

  let mut entries = Vec::new();
  for detail in raw_details.into_iter().flatten() {
    let victim_char_id = detail.victim.get("character_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let is_kill = victim_char_id != char_id;

    let ship_type_id = detail
      .victim
      .get("ship_type_id")
      .and_then(|v| v.as_i64())
      .map(|id| id as i32)
      .unwrap_or(0);

    let attacker_count = detail.attackers.len() as u32;

    let final_blow = detail.attackers.iter().any(|a| {
      a.get("final_blow").and_then(|v| v.as_bool()).unwrap_or(false)
        && a.get("character_id").and_then(|v| v.as_i64()).unwrap_or(0) == char_id
    });

    let victim_corp_id = detail
      .victim
      .get("corporation_id")
      .and_then(|v| v.as_i64())
      .unwrap_or(0);

    let victim_name = entity_name_map.get(&victim_char_id).cloned().unwrap_or_default();
    let victim_corp = entity_name_map.get(&victim_corp_id).cloned().unwrap_or_default();
    let total_value = zkill_value_map.get(&detail.killmail_id).copied().unwrap_or(0.0);

    let sys_id = detail.solar_system_id as i32;
    entries.push(pod_model::CharacterKillEntry {
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
    });
  }

  Ok(entries)
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
  let Some(token) = character_service::ensure_valid_token(&character, &esi, &db).await else {
    return Err("token refresh failed".into());
  };
  let grant = character_service::refresh_grant(&character, &token);
  let char_client = esi.character(&grant);

  let esi_notifications = char_client.notifications().await.map_err(|e| e.to_string())?;

  let notifications = esi_notifications
    .into_iter()
    .map(|n| {
      use pod_model::{NotificationCategory, categorize_notif};
      let cat_str = match categorize_notif(&n.r#type) {
        NotificationCategory::Alliance => "alliance",
        NotificationCategory::Clone => "clone",
        NotificationCategory::Combat => "combat",
        NotificationCategory::Contact => "contact",
        NotificationCategory::Contract => "contract",
        NotificationCategory::Corp => "corp",
        NotificationCategory::Fw => "fw",
        NotificationCategory::Incursion => "incursion",
        NotificationCategory::Industry => "industry",
        NotificationCategory::Insurance => "insurance",
        NotificationCategory::Market => "market",
        NotificationCategory::Mission => "mission",
        NotificationCategory::Reward => "reward",
        NotificationCategory::Standing => "standing",
        NotificationCategory::Structure => "structure",
        NotificationCategory::System => "system",
        NotificationCategory::War => "war",
      };
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
    })
    .collect();

  Ok(notifications)
}

async fn load_standings(
  character: Character,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Result<Vec<pod_model::CharacterStanding>, String> {
  let Some(token) = character_service::ensure_valid_token(&character, &esi, &db).await else {
    return Err("token refresh failed".into());
  };
  let grant = character_service::refresh_grant(&character, &token);
  let char_client = esi.character(&grant);

  let esi_standings = char_client.standings().await.map_err(|e| e.to_string())?;

  let standing_ids: Vec<i64> = esi_standings.iter().map(|s| s.from_id).collect();

  let standing_name_map: std::collections::HashMap<i64, String> = if standing_ids.is_empty() {
    std::collections::HashMap::new()
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
  let cached: std::collections::HashMap<i32, Vec<u8>> = db
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
      let _ = db.universe().type_icons().upsert(type_id, "icon", bytes.clone()).await;
      result.push((type_id, bytes));
    }
  }

  result
}
