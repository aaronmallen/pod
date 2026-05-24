//! Mail controller: three-pane EVE mail client.

use chrono::{Datelike, Utc, Weekday};
use iced::widget::image;
use pod_esi::models::character::{MailRecipient, NewMail, NewMailLabel, UpdateMail};
use pod_model::{Character, MailHeader};
use pod_ui::{
  components::{
    CharacterPicker,
    character_picker::{CharacterEntry, PickerSelection},
    compose_panel,
  },
  views::mail::{
    ComposeRecipient, DraggingPane, Folder, MailAccount, MailMessage, Message, State, folder_pane, message_list_pane,
    reading_pane,
  },
};

use crate::services::{Services, character as character_service};

/// Creates a new mail state and kicks off a background header fetch.
pub fn new(
  characters: Vec<Character>,
  services: &Services,
  folder_pane_width: f32,
  message_list_width: f32,
) -> (State, iced::Task<Message>) {
  let state = build_initial_mail_state(&characters, folder_pane_width, message_list_width);
  let task = if let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone()) {
    iced::Task::perform(
      async move { fetch_mail_headers(characters, esi, db).await },
      Message::MailHeadersLoaded,
    )
  } else {
    iced::Task::none()
  };
  (state, task)
}

/// Processes a mail message and returns a task.
pub fn update(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::AccountPicker(msg) => update_account_picker(state, msg),
    Message::FolderPane(folder_pane::Message::FolderSelected(folder)) => update_folder_selected(state, folder),
    Message::MessageList(msg) => update_message_list(state, msg, services),
    Message::ReadingPane(msg) => update_reading_pane(state, msg, services),
    msg => update_non_ui(state, msg, services),
  }
}

#[tracing::instrument(skip_all)]
async fn apply_label_to_mail(
  esi: &pod_esi::Client,
  grant: &pod_esi::models::auth::Grant,
  mail_id: i64,
  snoozed_id: i64,
  add_label: bool,
) -> Result<(), String> {
  let char_client = esi.character(grant);
  let mail_body = char_client
    .mail_message(mail_id)
    .await
    .map_err(|e| format!("Failed to fetch mail: {e}"))?;
  let mut mail_labels: Vec<i64> = mail_body.labels.unwrap_or_default();
  toggle_label_list(&mut mail_labels, snoozed_id, add_label);
  char_client
    .update_mail(
      mail_id,
      UpdateMail {
        labels: Some(mail_labels),
        read: None,
      },
    )
    .await
    .map_err(|e| format!("Failed to update mail labels: {e}"))
}

#[tracing::instrument(skip(characters, esi, db))]
async fn apply_snooze_label(
  character_id: i64,
  mail_id: i64,
  add_label: bool,
  characters: Vec<Character>,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Result<(), String> {
  let character = characters
    .iter()
    .find(|c| *c.id() == character_id)
    .ok_or_else(|| "Character not found".to_string())?;
  let Some(token) = character_service::ensure_valid_token(character, &esi, &db).await else {
    return Err("Token refresh failed".to_string());
  };
  let grant = character_service::refresh_grant(character, &token);
  let Some(snoozed_id) = get_or_create_snoozed_label(&esi, &grant, add_label).await? else {
    return Ok(());
  };
  apply_label_to_mail(&esi, &grant, mail_id, snoozed_id, add_label).await
}

fn build_accounts(characters: &[Character]) -> Vec<MailAccount> {
  characters
    .iter()
    .map(|c| MailAccount {
      id: *c.id(),
      name: c.name().clone(),
      corp: c.corp_name().clone(),
      tone: *c.portrait_tone() as u16,
      unread: 0,
    })
    .collect()
}

fn build_db_mail_rows(
  esi_headers: &[pod_esi::models::character::MailHeader],
  recipient_map: &std::collections::HashMap<i64, Vec<i64>>,
  name_map: &std::collections::HashMap<i64, String>,
  character_id: i64,
) -> Vec<MailHeader> {
  esi_headers
    .iter()
    .filter_map(|h| {
      let mail_id = h.mail_id?;
      let is_sent = h.from == Some(character_id);
      let recipients_display = if is_sent {
        recipient_map
          .get(&mail_id)
          .map(|ids| {
            ids
              .iter()
              .map(|id| {
                name_map
                  .get(id)
                  .cloned()
                  .expect("recipient name must be resolved by ESI")
              })
              .collect::<Vec<_>>()
              .join(", ")
          })
          .unwrap_or_default()
      } else {
        String::new()
      };
      Some(MailHeader {
        character_id,
        mail_id,
        subject: h.subject.clone().unwrap_or_default(),
        from_id: h.from,
        is_read: h.is_read.unwrap_or(false),
        timestamp: h.timestamp.clone().unwrap_or_default(),
        recipients_display,
      })
    })
    .collect()
}

fn build_initial_mail_state(characters: &[Character], folder_pane_width: f32, message_list_width: f32) -> State {
  let first_id = characters.first().map(|c| *c.id()).unwrap_or(0);
  let accounts = build_accounts(characters);
  let portrait_handles = build_portrait_handles(characters);
  let picker_entries = build_picker_entries_for_accounts(&accounts, &portrait_handles);
  let account_picker = CharacterPicker::new()
    .entries(picker_entries.clone())
    .selected(PickerSelection::Character(first_id));
  let compose = pod_ui::components::ComposePanel::new()
    .from_entries(picker_entries)
    .from_selected(Some(first_id));
  State {
    accounts,
    characters: characters.to_vec(),
    portrait_handles,
    account_picker,
    messages: Vec::new(),
    selected_folder: Folder::Inbox,
    selected_message_id: None,
    search_query: String::new(),
    compose_open: false,
    compose,
    snooze_popover_open: false,
    folder_pane_width,
    last_drag_x: 0.0,
    message_list_width,
    dragging_pane: None,
    context_menu: None,
    cursor_pos: (0.0, 0.0),
  }
}

fn build_mail_message(
  row: MailHeader,
  name_map: &std::collections::HashMap<i64, String>,
  character_name: &str,
  snoozed_lookup: &std::collections::HashMap<(i64, i64), String>,
) -> MailMessage {
  let is_sent = row.from_id == Some(row.character_id);
  let from_name = if is_sent {
    character_name.to_string()
  } else {
    row
      .from_id
      .map(|id| name_map.get(&id).cloned().expect("sender name must be resolved by ESI"))
      .unwrap_or_default()
  };
  let snoozed = snoozed_lookup.get(&(row.character_id, row.mail_id)).cloned();
  MailMessage {
    character_id: row.character_id,
    mail_id: row.mail_id,
    from_id: row.from_id,
    id: format!("{}-{}", row.character_id, row.mail_id),
    folder: if is_sent { "sent" } else { "inbox" }.to_string(),
    from_name,
    from_tone: 0,
    from_corp: false,
    from_system: false,
    subject: row.subject,
    preview: String::new(),
    body: Vec::new(),
    body_loaded: false,
    time: format_timestamp_label(&row.timestamp),
    date_label: date_bucket_label(&row.timestamp),
    unread: !row.is_read && !is_sent,
    starred: false,
    pinned: false,
    has_attachment: false,
    labels: Vec::new(),
    important: false,
    snoozed,
    recipients_display: row.recipients_display,
  }
}

fn build_picker_entries_for_accounts(
  accounts: &[MailAccount],
  portrait_handles: &std::collections::HashMap<i64, image::Handle>,
) -> Vec<CharacterEntry> {
  accounts
    .iter()
    .map(|a| CharacterEntry {
      id: Some(a.id),
      name: a.name.clone(),
      corp_name: a.corp.clone(),
      tone: a.tone,
      portrait_handle: portrait_handles.get(&a.id).cloned(),
    })
    .collect()
}

fn build_portrait_handles(characters: &[Character]) -> std::collections::HashMap<i64, image::Handle> {
  characters
    .iter()
    .filter_map(|c| {
      c.portrait_data()
        .as_ref()
        .map(|b| (*c.id(), image::Handle::from_bytes(b.clone())))
    })
    .collect()
}

fn build_recipient_map(
  esi_headers: &[pod_esi::models::character::MailHeader],
) -> std::collections::HashMap<i64, Vec<i64>> {
  let mut map = std::collections::HashMap::new();
  for h in esi_headers {
    if let (Some(mail_id), Some(recipients)) = (h.mail_id, &h.recipients) {
      let ids: Vec<i64> = recipients.iter().map(|r| r.recipient_id).collect();
      if !ids.is_empty() {
        map.insert(mail_id, ids);
      }
    }
  }
  map
}

#[tracing::instrument(skip_all)]
async fn check_expired_snoozes(
  characters: Vec<Character>,
  esi: Option<pod_esi::Client>,
  db: pod_db::Repo,
) -> Vec<(i64, i64)> {
  let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
  let expired = match db.characters().expired_snoozed_mails(&now).await {
    Ok(rows) => rows,
    Err(_) => return Vec::new(),
  };
  let mut unsnooze_pairs: Vec<(i64, i64)> = Vec::new();
  for row in expired {
    // best-effort; snooze re-expires on the next sync cycle if this fails
    let _ = db.characters().delete_snoozed_mail(row.character_id, row.mail_id).await;
    if let Some(esi) = &esi {
      let chars = characters.clone();
      // best-effort; snooze re-expires on the next sync cycle if this fails
      let _ = apply_snooze_label(row.character_id, row.mail_id, false, chars, esi.clone(), db.clone()).await;
    }
    unsnooze_pairs.push((row.character_id, row.mail_id));
  }
  unsnooze_pairs
}

fn collect_name_ids(
  esi_headers: &[pod_esi::models::character::MailHeader],
  recipient_map: &std::collections::HashMap<i64, Vec<i64>>,
  character_id: i64,
) -> Vec<i64> {
  let mut seen = std::collections::HashSet::new();
  let mut ids = Vec::new();
  for h in esi_headers {
    if let Some(id) = h.from
      && id != character_id
      && seen.insert(id)
    {
      ids.push(id);
    }
  }
  for recipient_ids in recipient_map.values() {
    for &id in recipient_ids {
      if id != character_id && seen.insert(id) {
        ids.push(id);
      }
    }
  }
  ids
}

fn date_bucket_label(ts: &str) -> String {
  if ts.len() >= 10 {
    ts[..10].to_string()
  } else {
    ts.to_string()
  }
}

#[tracing::instrument(skip(characters, esi, db))]
async fn fetch_mail_body(
  msg_id: String,
  character_id: i64,
  mail_id: i64,
  characters: Vec<Character>,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> (String, Vec<String>) {
  let Some(character) = characters.iter().find(|c| *c.id() == character_id) else {
    return (msg_id, Vec::new());
  };
  let Some(token) = character_service::ensure_valid_token(character, &esi, &db).await else {
    return (msg_id, Vec::new());
  };
  let grant = character_service::refresh_grant(character, &token);
  let char_client = esi.character(&grant);
  match char_client.mail_message(mail_id).await {
    Ok(esi_msg) => {
      let html = esi_msg.body.unwrap_or_default();
      (msg_id, strip_html(&html))
    }
    Err(e) => {
      tracing::warn!("mail: failed to fetch body for mail {mail_id}: {e}");
      (msg_id, Vec::new())
    }
  }
}

#[tracing::instrument(skip_all)]
async fn fetch_character_mail(
  character: &Character,
  esi: &pod_esi::Client,
  db: &pod_db::Repo,
  snoozed_lookup: &std::collections::HashMap<(i64, i64), String>,
) -> Result<Vec<MailMessage>, String> {
  let Some(token) = character_service::ensure_valid_token(character, esi, db).await else {
    return Ok(Vec::new());
  };
  let grant = character_service::refresh_grant(character, &token);
  let char_client = esi.character(&grant);
  let esi_headers = char_client.mail().await.unwrap_or_default();
  let recipient_map = build_recipient_map(&esi_headers);
  let name_ids = collect_name_ids(&esi_headers, &recipient_map, *character.id());
  let mut name_map = resolve_mail_name_map(&name_ids, esi).await?;
  if !esi_headers.is_empty() {
    let db_rows = build_db_mail_rows(&esi_headers, &recipient_map, &name_map, *character.id());
    // best-effort cache write
    let _ = db.characters().upsert_mail_headers(*character.id(), &db_rows).await;
  }
  let Ok(rows) = db.characters().mail_headers(*character.id()).await else {
    return Ok(Vec::new());
  };
  supplement_name_map(&rows, &mut name_map, *character.id(), esi).await?;
  Ok(
    rows
      .into_iter()
      .map(|row| build_mail_message(row, &name_map, character.name(), snoozed_lookup))
      .collect(),
  )
}

#[tracing::instrument(skip_all)]
async fn fetch_mail_headers(
  characters: Vec<Character>,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Result<Vec<MailMessage>, String> {
  let mut all: Vec<MailMessage> = Vec::new();
  let snoozed_rows = db.characters().all_snoozed_mails().await.unwrap_or_default();
  let snoozed_lookup: std::collections::HashMap<(i64, i64), String> = snoozed_rows
    .into_iter()
    .map(|r| ((r.character_id, r.mail_id), r.snooze_until))
    .collect();
  for character in &characters {
    let mut messages = fetch_character_mail(character, &esi, &db, &snoozed_lookup).await?;
    all.append(&mut messages);
  }
  Ok(all)
}

fn find_snoozed_label_id(all_labels: &pod_esi::models::character::MailLabels) -> Option<i64> {
  all_labels
    .labels
    .as_deref()
    .unwrap_or_default()
    .iter()
    .find(|l| l.name.as_deref() == Some("Snoozed"))
    .and_then(|l| l.label_id)
}

fn format_timestamp_label(ts: &str) -> String {
  if ts.len() >= 16 {
    ts[11..16].to_string()
  } else {
    ts.to_string()
  }
}

async fn get_or_create_snoozed_label(
  esi: &pod_esi::Client,
  grant: &pod_esi::models::auth::Grant,
  add_label: bool,
) -> Result<Option<i64>, String> {
  let char_client = esi.character(grant);
  let all_labels = char_client
    .mail_labels()
    .await
    .map_err(|e| format!("Failed to fetch labels: {e}"))?;
  match find_snoozed_label_id(&all_labels) {
    Some(id) => Ok(Some(id)),
    None if !add_label => Ok(None),
    None => {
      let id = char_client
        .create_mail_label(NewMailLabel {
          color: Some("#ffaa00".into()),
          name: "Snoozed".into(),
        })
        .await
        .map_err(|e| format!("Failed to create Snoozed label: {e}"))?;
      Ok(Some(id))
    }
  }
}

fn handle_compose_search(
  val: String,
  state: &mut State,
  services: &Services,
  wrap: fn(Vec<(i64, String)>) -> compose_panel::Message,
) -> iced::Task<Message> {
  if val.trim().len() >= 3
    && let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone())
  {
    let chars = state.characters.clone();
    return iced::Task::perform(
      async move { search_recipients(val, chars, esi, db).await },
      move |results| Message::Compose(wrap(results)),
    );
  }
  iced::Task::none()
}

fn prepare_and_send_mail(
  state: &mut State,
  compose_msg: compose_panel::Message,
  services: &Services,
) -> iced::Task<Message> {
  if state.compose.to.is_empty() || state.compose.subject.trim().is_empty() {
    return iced::Task::none();
  }
  if state.compose.sending {
    return iced::Task::none();
  }
  let Some(esi) = services.esi_client.clone() else {
    state.compose.error = Some("Not connected to ESI".to_string());
    return iced::Task::none();
  };
  let Some(db) = services.db.clone() else {
    state.compose.error = Some("Database unavailable".to_string());
    return iced::Task::none();
  };
  let from_id = state.compose.from_picker.selected_character_id().unwrap_or(0);
  let to = state.compose.to.clone();
  let cc = state.compose.cc.clone();
  let subject = state.compose.subject.clone();
  let body = state.compose.body.text();
  let characters = state.characters.clone();
  state.compose.update(compose_msg);
  iced::Task::perform(
    async move { send_composed_mail(from_id, to, cc, subject, body, characters, esi, db).await },
    |result| Message::Compose(compose_panel::Message::Sent(result)),
  )
}

fn push_resolved_ids(vals: &[serde_json::Value], rtype: &str, out: &mut Vec<MailRecipient>) {
  for v in vals {
    if let Some(id) = v.get("id").and_then(|x| x.as_i64()) {
      out.push(MailRecipient {
        recipient_id: id,
        recipient_type: rtype.to_string(),
      });
    }
  }
}

fn recompute_unread_counts(state: &mut State) {
  let mut unread_by_char: std::collections::HashMap<i64, u32> = std::collections::HashMap::new();
  for m in &state.messages {
    if m.unread && m.folder != "sent" {
      *unread_by_char.entry(m.character_id).or_insert(0) += 1;
    }
  }
  for acct in &mut state.accounts {
    acct.unread = *unread_by_char.get(&acct.id).unwrap_or(&0);
  }
}

async fn resolve_all_recipients(
  all: &[&ComposeRecipient],
  esi: &pod_esi::Client,
) -> Result<Vec<MailRecipient>, String> {
  let mut resolved: Vec<MailRecipient> = Vec::new();
  let mut needs_resolution: Vec<&str> = Vec::new();
  for r in all {
    if let Some(id) = r.id {
      resolved.push(MailRecipient {
        recipient_id: id,
        recipient_type: "character".to_string(),
      });
    } else {
      needs_resolution.push(r.name.as_str());
    }
  }
  if !needs_resolution.is_empty() {
    let named = resolve_named_recipients(&needs_resolution, esi).await?;
    resolved.extend(named);
  }
  Ok(resolved)
}

async fn resolve_mail_name_map(
  name_ids: &[i64],
  esi: &pod_esi::Client,
) -> Result<std::collections::HashMap<i64, String>, String> {
  if name_ids.is_empty() {
    return Ok(std::collections::HashMap::new());
  }
  esi
    .universe()
    .names(name_ids)
    .await
    .map_err(|e| format!("ESI name resolution failed: {e}"))
    .map(|ns| ns.into_iter().map(|n| (n.id, n.name)).collect())
}

async fn resolve_named_recipients(names: &[&str], esi: &pod_esi::Client) -> Result<Vec<MailRecipient>, String> {
  let ids_result = esi
    .universe()
    .ids(names)
    .await
    .map_err(|e| format!("Name resolution failed: {e}"))?;
  let mut out = Vec::new();
  if let Some(chars) = &ids_result.characters {
    push_resolved_ids(chars, "character", &mut out);
  }
  if let Some(corps) = &ids_result.corporations {
    push_resolved_ids(corps, "corporation", &mut out);
  }
  if let Some(alliances) = &ids_result.alliances {
    push_resolved_ids(alliances, "alliance", &mut out);
  }
  Ok(out)
}

#[tracing::instrument(skip(characters, esi, db))]
async fn search_recipients(
  query: String,
  characters: Vec<Character>,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Vec<(i64, String)> {
  let Some(character) = characters.first() else {
    return Vec::new();
  };
  let Some(token) = character_service::ensure_valid_token(character, &esi, &db).await else {
    return Vec::new();
  };
  let grant = character_service::refresh_grant(character, &token);
  let char_client = esi.character(&grant);

  let ids = match char_client.search_characters(&query).await {
    Ok(ids) => ids,
    Err(e) => {
      tracing::warn!("mail: recipient search failed for query {query:?}: {e}");
      return Vec::new();
    }
  };
  if ids.is_empty() {
    return Vec::new();
  }

  let ids_limited: Vec<i64> = ids.into_iter().take(20).collect();
  match esi.universe().names(&ids_limited).await {
    Ok(names) => names
      .into_iter()
      .filter(|n| n.category == "character")
      .map(|n| (n.id, n.name))
      .collect(),
    Err(_) => Vec::new(),
  }
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(skip(characters, esi, db))]
async fn send_composed_mail(
  from_id: i64,
  to: Vec<ComposeRecipient>,
  cc: Vec<ComposeRecipient>,
  subject: String,
  body: String,
  characters: Vec<Character>,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Result<i64, String> {
  let character = characters
    .iter()
    .find(|c| *c.id() == from_id)
    .ok_or_else(|| "Sending character not found".to_string())?;
  let Some(token) = character_service::ensure_valid_token(character, &esi, &db).await else {
    return Err("Failed to refresh auth token".to_string());
  };
  let grant = character_service::refresh_grant(character, &token);
  let char_client = esi.character(&grant);
  let all_recipients: Vec<&ComposeRecipient> = to.iter().chain(cc.iter()).collect();
  if all_recipients.is_empty() {
    return Err("No recipients specified".to_string());
  }
  let resolved = resolve_all_recipients(&all_recipients, &esi).await?;
  if resolved.is_empty() {
    return Err("No valid recipients found — check character/corporation names".to_string());
  }
  tracing::info!(
    "mail: sending — from_id: {from_id}, to {} recipient(s), subject: {subject}",
    resolved.len()
  );
  char_client
    .send_mail(NewMail {
      approved_cost: None,
      body,
      recipients: resolved,
      subject,
    })
    .await
    .map_err(|e| format!("Send failed: {e}"))
}

fn snooze_after_downtime(now: chrono::DateTime<Utc>, today: chrono::NaiveDate) -> Option<chrono::DateTime<Utc>> {
  let downtime = today.and_hms_opt(11, 0, 0)?.and_utc();
  if now >= downtime {
    Some(today.succ_opt()?.and_hms_opt(11, 0, 0)?.and_utc())
  } else {
    Some(downtime)
  }
}

fn snooze_next_monday(today: chrono::NaiveDate) -> Option<chrono::DateTime<Utc>> {
  let days_until_mon = match today.weekday() {
    Weekday::Mon => 7,
    d => (8 - d.number_from_monday() as i64).rem_euclid(7).max(1),
  };
  (today + chrono::Duration::days(days_until_mon))
    .and_hms_opt(9, 0, 0)
    .map(|dt| dt.and_utc())
}

fn snooze_label_to_iso(label: &str) -> Option<String> {
  let now = Utc::now();
  let today = now.date_naive();
  let target = match label {
    "Later today" => today.and_hms_opt(18, 0, 0)?.and_utc(),
    "Tomorrow" => today.succ_opt()?.and_hms_opt(9, 0, 0)?.and_utc(),
    "After downtime" => snooze_after_downtime(now, today)?,
    "Next week" => snooze_next_monday(today)?,
    _ => return None,
  };
  Some(target.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

fn strip_html(html: &str) -> Vec<String> {
  let s = html
    .replace("<br>", "\n")
    .replace("<br/>", "\n")
    .replace("<br />", "\n")
    .replace("<BR>", "\n")
    .replace("<BR/>", "\n")
    .replace("<p>", "\n")
    .replace("</p>", "\n")
    .replace("<P>", "\n")
    .replace("</P>", "\n")
    .replace("&amp;", "&")
    .replace("&lt;", "<")
    .replace("&gt;", ">")
    .replace("&nbsp;", " ")
    .replace("&#13;", "\n")
    .replace("&#10;", "\n");
  let mut result = String::new();
  let mut in_tag = false;
  for ch in s.chars() {
    match ch {
      '<' => in_tag = true,
      '>' => in_tag = false,
      _ if !in_tag => result.push(ch),
      _ => {}
    }
  }
  result
    .split('\n')
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .map(str::to_string)
    .collect()
}

async fn supplement_name_map(
  rows: &[MailHeader],
  name_map: &mut std::collections::HashMap<i64, String>,
  character_id: i64,
  esi: &pod_esi::Client,
) -> Result<(), String> {
  let extra_ids: Vec<i64> = rows
    .iter()
    .filter_map(|r| r.from_id)
    .filter(|id| *id != character_id && !name_map.contains_key(id))
    .collect::<std::collections::HashSet<_>>()
    .into_iter()
    .collect();
  if extra_ids.is_empty() {
    return Ok(());
  }
  let ns = esi
    .universe()
    .names(&extra_ids)
    .await
    .map_err(|e| format!("ESI name resolution failed: {e}"))?;
  for n in ns {
    name_map.insert(n.id, n.name);
  }
  let still_missing: Vec<i64> = extra_ids.into_iter().filter(|id| !name_map.contains_key(id)).collect();
  if still_missing.is_empty() {
    Ok(())
  } else {
    Err(format!("could not resolve ESI names for mail IDs: {still_missing:?}"))
  }
}

fn toggle_label_list(labels: &mut Vec<i64>, label_id: i64, add: bool) {
  if add {
    if !labels.contains(&label_id) {
      labels.push(label_id);
    }
  } else {
    labels.retain(|&id| id != label_id);
  }
}

fn update_account_picker(state: &mut State, msg: pod_ui::components::character_picker::Message) -> iced::Task<Message> {
  state.account_picker.update(msg);
  iced::Task::none()
}

fn update_archive(state: &mut State) -> iced::Task<Message> {
  state.context_menu = None;
  if let Some(id) = state.selected_message_id.clone() {
    tracing::info!("mail: message archived — {id}");
    if let Some(msg) = state.messages.iter_mut().find(|m| m.id == id) {
      msg.folder = "archive".to_string();
    }
    state.selected_message_id = None;
  }
  iced::Task::none()
}

fn update_check_snoozed(state: &mut State, services: &Services) -> iced::Task<Message> {
  if let Some(db) = services.db.clone() {
    let chars = state.characters.clone();
    let esi = services.esi_client.clone();
    return iced::Task::perform(async move { check_expired_snoozes(chars, esi, db).await }, |expired| {
      Message::ReadingPane(reading_pane::Message::SnoozedExpired(expired))
    });
  }
  iced::Task::none()
}

fn update_compose(state: &mut State, compose_msg: compose_panel::Message, services: &Services) -> iced::Task<Message> {
  match &compose_msg {
    compose_panel::Message::Close => {
      state.compose_open = false;
      state.compose.update(compose_msg);
      iced::Task::none()
    }
    compose_panel::Message::ToSearchChanged(val) => {
      let val = val.clone();
      state.compose.update(compose_msg);
      handle_compose_search(val, state, services, compose_panel::Message::ToSearchResults)
    }
    compose_panel::Message::CcSearchChanged(val) => {
      let val = val.clone();
      state.compose.update(compose_msg);
      handle_compose_search(val, state, services, compose_panel::Message::CcSearchResults)
    }
    compose_panel::Message::SendPressed => {
      tracing::info!("mail: send pressed");
      prepare_and_send_mail(state, compose_msg, services)
    }
    compose_panel::Message::Sent(Ok(mail_id)) => {
      tracing::info!("mail: mail sent — mail_id: {mail_id}");
      state.compose_open = false;
      state.compose.update(compose_msg);
      iced::Task::none()
    }
    compose_panel::Message::Sent(Err(e)) => {
      tracing::warn!("mail: send failed — {e}");
      state.compose.update(compose_msg);
      iced::Task::none()
    }
    _ => {
      state.compose.update(compose_msg);
      iced::Task::none()
    }
  }
}

fn update_compose_open(state: &mut State) -> iced::Task<Message> {
  tracing::info!("mail: compose opened");
  let from_id = state.account_picker.selected.clone();
  state.compose_open = true;
  state.compose.reset();
  state.compose.from_picker.selected = from_id;
  iced::Task::none()
}

fn update_delete(state: &mut State, services: &Services) -> iced::Task<Message> {
  state.context_menu = None;
  if let Some(id) = state.selected_message_id.take()
    && let Some(pos) = state.messages.iter().position(|m| m.id == id)
  {
    let mail_id = state.messages[pos].mail_id;
    let character_id = state.messages[pos].character_id;
    tracing::info!("mail: delete requested — character_id: {character_id}, mail_id: {mail_id}");
    state.messages.remove(pos);
    state.selected_message_id = state
      .messages
      .get(pos)
      .or_else(|| state.messages.last())
      .map(|m| m.id.clone());
    recompute_unread_counts(state);
    if let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone()) {
      let chars = state.characters.clone();
      return iced::Task::perform(
        async move {
          // best-effort; mail may reappear until next sync if either fails
          let _ = db.characters().delete_mail_header(character_id, mail_id).await;
          if let Some(character) = chars.iter().find(|c| *c.id() == character_id)
            && let Some(token) = character_service::ensure_valid_token(character, &esi, &db).await
          {
            let grant = character_service::refresh_grant(character, &token);
            // best-effort; mail may reappear until next sync if either fails
            let _ = esi.character(&grant).delete_mail(mail_id).await;
          }
        },
        |_| Message::MailDeleted,
      );
    }
  }
  iced::Task::none()
}

fn update_folder_selected(state: &mut State, folder: Folder) -> iced::Task<Message> {
  tracing::info!("mail: folder selected — {folder:?}");
  state.selected_folder = folder;
  state.selected_message_id = None;
  iced::Task::none()
}

fn update_forward(state: &mut State) -> iced::Task<Message> {
  state.context_menu = None;
  let Some(msg) = state
    .selected_message_id
    .as_ref()
    .and_then(|id| state.messages.iter().find(|m| &m.id == id))
  else {
    return iced::Task::none();
  };
  let subject = if msg.subject.starts_with("Fwd: ") {
    msg.subject.clone()
  } else {
    format!("Fwd: {}", msg.subject)
  };
  let body_text = if msg.body.is_empty() {
    String::new()
  } else {
    format!("\n\n--- Forwarded message ---\n{}", msg.body.join("\n"))
  };
  tracing::info!(
    "mail: forward initiated — character_id: {}, subject: {subject}",
    msg.character_id
  );
  state.compose_open = true;
  state.compose.reset();
  state.compose.subject = subject;
  state.compose.body = iced::widget::text_editor::Content::with_text(&body_text);
  state.compose.from_picker.selected = PickerSelection::Character(msg.character_id);
  state.snooze_popover_open = false;
  iced::Task::none()
}

fn update_mail_body_loaded(state: &mut State, msg_id: String, paragraphs: Vec<String>) -> iced::Task<Message> {
  tracing::debug!("mail: body loaded — {msg_id}");
  if let Some(msg) = state.messages.iter_mut().find(|m| m.id == msg_id) {
    msg.body = paragraphs;
    msg.body_loaded = true;
    msg.unread = false;
  }
  iced::Task::none()
}

fn update_mail_deleted(state: &mut State) -> iced::Task<Message> {
  recompute_unread_counts(state);
  iced::Task::none()
}

fn update_mail_headers_loaded(state: &mut State, result: Result<Vec<MailMessage>, String>) -> iced::Task<Message> {
  let messages = match result {
    Ok(m) => m,
    Err(e) => {
      tracing::warn!("mail: failed to load headers: {e}");
      return iced::Task::none();
    }
  };
  tracing::debug!("mail: {} header(s) loaded", messages.len());
  let mut unread_by_char: std::collections::HashMap<i64, u32> = std::collections::HashMap::new();
  for m in &messages {
    if m.unread && m.folder != "sent" {
      *unread_by_char.entry(m.character_id).or_insert(0) += 1;
    }
  }
  for acct in &mut state.accounts {
    acct.unread = *unread_by_char.get(&acct.id).unwrap_or(&0);
  }
  state.selected_message_id = messages.first().map(|m| m.id.clone());
  state.messages = messages;
  iced::Task::none()
}

fn update_message_list(state: &mut State, msg: message_list_pane::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    message_list_pane::Message::ContextMenuClose => {
      state.context_menu = None;
      iced::Task::none()
    }
    message_list_pane::Message::CursorMoved(x, y) => {
      state.cursor_pos = (x, y);
      iced::Task::none()
    }
    message_list_pane::Message::MessageRightClicked(id) => update_message_right_clicked(state, id),
    message_list_pane::Message::MessageSelected(id) => update_message_selected(state, id, services),
    message_list_pane::Message::SearchChanged(q) => {
      state.search_query = q;
      iced::Task::none()
    }
  }
}

fn update_message_right_clicked(state: &mut State, id: String) -> iced::Task<Message> {
  state.context_menu = Some((id.clone(), state.cursor_pos.0, state.cursor_pos.1));
  state.selected_message_id = Some(id);
  state.snooze_popover_open = false;
  iced::Task::none()
}

fn update_message_selected(state: &mut State, id: String, services: &Services) -> iced::Task<Message> {
  tracing::info!("mail: message selected — {id}");
  state.context_menu = None;
  state.snooze_popover_open = false;
  state.selected_message_id = Some(id.clone());
  if let Some(msg) = state.messages.iter().find(|m| m.id == id)
    && msg.body.is_empty()
  {
    let char_id = msg.character_id;
    let mail_id = msg.mail_id;
    if let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone()) {
      let chars = state.characters.clone();
      return iced::Task::perform(
        async move { fetch_mail_body(id, char_id, mail_id, chars, esi, db).await },
        |(msg_id, body)| Message::MailBodyLoaded(msg_id, body),
      );
    }
  }
  iced::Task::none()
}

fn update_non_ui(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::ComposePressed => update_compose_open(state),
    Message::Compose(compose_msg) => update_compose(state, compose_msg, services),
    Message::MailBodyLoaded(msg_id, paragraphs) => update_mail_body_loaded(state, msg_id, paragraphs),
    Message::MailDeleted => update_mail_deleted(state),
    msg => update_pane_and_headers(state, msg),
  }
}

fn update_pane_and_headers(state: &mut State, message: Message) -> iced::Task<Message> {
  match message {
    Message::MailHeadersLoaded(messages) => update_mail_headers_loaded(state, messages),
    Message::PaneDrag(x) => update_pane_drag(state, x),
    Message::PaneDragEnd => update_pane_drag_end(state),
    Message::PaneDragStart(pane) => update_pane_drag_start(state, pane),
    _ => iced::Task::none(),
  }
}

fn update_pane_drag(state: &mut State, x: f32) -> iced::Task<Message> {
  if state.last_drag_x > 0.0 {
    let delta = x - state.last_drag_x;
    match state.dragging_pane {
      Some(DraggingPane::FolderList) => {
        state.folder_pane_width = (state.folder_pane_width + delta).max(80.0);
      }
      Some(DraggingPane::MessageReader) => {
        state.message_list_width = (state.message_list_width + delta).max(100.0);
      }
      None => {}
    }
  }
  state.last_drag_x = x;
  iced::Task::none()
}

fn update_pane_drag_end(state: &mut State) -> iced::Task<Message> {
  state.dragging_pane = None;
  state.last_drag_x = 0.0;
  iced::Task::none()
}

fn update_pane_drag_start(state: &mut State, pane: DraggingPane) -> iced::Task<Message> {
  state.dragging_pane = Some(pane);
  state.last_drag_x = 0.0;
  iced::Task::none()
}

fn update_reading_pane(state: &mut State, msg: reading_pane::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    reading_pane::Message::ArchivePressed => update_archive(state),
    reading_pane::Message::ForwardPressed => update_forward(state),
    reading_pane::Message::ReplyAllPressed => update_reply(state),
    reading_pane::Message::ReplyPressed => update_reply(state),
    reading_pane::Message::StarToggle => update_star_toggle(state),
    msg => update_snooze_message(state, msg, services),
  }
}

fn update_reply(state: &mut State) -> iced::Task<Message> {
  state.context_menu = None;
  let Some(msg) = state
    .selected_message_id
    .as_ref()
    .and_then(|id| state.messages.iter().find(|m| &m.id == id))
  else {
    return iced::Task::none();
  };
  let to = ComposeRecipient {
    name: msg.from_name.clone(),
    id: msg.from_id,
  };
  let subject = if msg.subject.starts_with("Re: ") {
    msg.subject.clone()
  } else {
    format!("Re: {}", msg.subject)
  };
  tracing::info!(
    "mail: reply initiated — character_id: {}, subject: {subject}",
    msg.character_id
  );
  state.compose_open = true;
  state.compose.reset();
  state.compose.to = vec![to];
  state.compose.subject = subject;
  state.compose.from_picker.selected = PickerSelection::Character(msg.character_id);
  state.snooze_popover_open = false;
  iced::Task::none()
}

fn update_snooze_message(state: &mut State, msg: reading_pane::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    reading_pane::Message::CheckSnoozed => update_check_snoozed(state, services),
    reading_pane::Message::DeletePressed => update_delete(state, services),
    reading_pane::Message::SnoozedExpired(pairs) => update_snoozed_expired(state, pairs),
    reading_pane::Message::SnoozeSet(label) => update_snooze_set(state, label, services),
    reading_pane::Message::SnoozeFailed(e) => {
      tracing::warn!("mail: snooze operation failed — {e}");
      iced::Task::none()
    }
    reading_pane::Message::SnoozeToggle => update_snooze_toggle(state),
    _ => iced::Task::none(),
  }
}

fn update_snooze_set(state: &mut State, label: String, services: &Services) -> iced::Task<Message> {
  state.context_menu = None;
  state.snooze_popover_open = false;
  let Some(id) = state.selected_message_id.clone() else {
    return iced::Task::none();
  };
  let Some(msg) = state.messages.iter_mut().find(|m| m.id == id) else {
    return iced::Task::none();
  };
  let character_id = msg.character_id;
  let mail_id = msg.mail_id;
  let adding = !label.is_empty();
  if adding {
    tracing::info!("mail: snooze set — character_id: {character_id}, mail_id: {mail_id}, label: {label}");
  } else {
    tracing::info!("mail: snooze removed — character_id: {character_id}, mail_id: {mail_id}");
  }
  let snooze_until = if adding { snooze_label_to_iso(&label) } else { None };
  if adding {
    msg.snoozed = Some(label);
  } else {
    msg.snoozed = None;
  }
  if let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone()) {
    let chars = state.characters.clone();
    return iced::Task::perform(
      async move {
        if adding {
          if let Some(until) = &snooze_until {
            // best-effort; snooze state may be inconsistent until next launch if this fails
            let _ = db.characters().upsert_snoozed_mail(character_id, mail_id, until).await;
          }
        } else {
          // best-effort; snooze state may be inconsistent until next launch if this fails
          let _ = db.characters().delete_snoozed_mail(character_id, mail_id).await;
        }
        apply_snooze_label(character_id, mail_id, adding, chars, esi, db).await
      },
      |res| match res {
        Ok(()) => Message::MailDeleted,
        Err(e) => Message::ReadingPane(reading_pane::Message::SnoozeFailed(e)),
      },
    );
  }
  iced::Task::none()
}

fn update_snooze_toggle(state: &mut State) -> iced::Task<Message> {
  state.context_menu = None;
  state.snooze_popover_open = !state.snooze_popover_open;
  iced::Task::none()
}

fn update_snoozed_expired(state: &mut State, pairs: Vec<(i64, i64)>) -> iced::Task<Message> {
  tracing::debug!("mail: {} snoozed mail(s) expired and restored to inbox", pairs.len());
  for (character_id, mail_id) in pairs {
    let msg_id = format!("{character_id}-{mail_id}");
    if let Some(msg) = state.messages.iter_mut().find(|m| m.id == msg_id) {
      msg.snoozed = None;
      msg.unread = true;
      msg.folder = "inbox".to_string();
    }
  }
  iced::Task::none()
}

fn update_star_toggle(state: &mut State) -> iced::Task<Message> {
  state.context_menu = None;
  if let Some(id) = state.selected_message_id.clone()
    && let Some(msg) = state.messages.iter_mut().find(|m| m.id == id)
  {
    msg.starred = !msg.starred;
    tracing::info!("mail: star toggled — {id}, starred: {}", msg.starred);
  }
  iced::Task::none()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod date_bucket_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_extracts_date_portion_from_iso_timestamp() {
      let result = date_bucket_label("2024-01-15T14:32:00Z");

      assert_eq!(result, "2024-01-15");
    }

    #[test]
    fn it_returns_short_strings_unchanged() {
      let result = date_bucket_label("2024");

      assert_eq!(result, "2024");
    }
  }

  mod find_snoozed_label_id {
    use pod_esi::models::character::{MailLabel, MailLabels};
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_none_when_labels_list_is_empty() {
      let ml = MailLabels {
        labels: None,
        total_unread_count: None,
      };

      assert_eq!(find_snoozed_label_id(&ml), None);
    }

    #[test]
    fn it_returns_none_when_snoozed_label_is_absent() {
      let ml = MailLabels {
        labels: Some(vec![MailLabel {
          color: None,
          label_id: Some(1),
          name: Some("Other".into()),
          unread_count: None,
        }]),
        total_unread_count: None,
      };

      assert_eq!(find_snoozed_label_id(&ml), None);
    }

    #[test]
    fn it_returns_the_label_id_when_snoozed_label_exists() {
      let ml = MailLabels {
        labels: Some(vec![MailLabel {
          color: None,
          label_id: Some(42),
          name: Some("Snoozed".into()),
          unread_count: None,
        }]),
        total_unread_count: None,
      };

      assert_eq!(find_snoozed_label_id(&ml), Some(42));
    }
  }

  mod format_timestamp_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_extracts_hhmm_from_iso_timestamp() {
      let result = format_timestamp_label("2024-01-15T14:32:00Z");

      assert_eq!(result, "14:32");
    }

    #[test]
    fn it_returns_short_strings_unchanged() {
      let result = format_timestamp_label("2024");

      assert_eq!(result, "2024");
    }
  }

  mod push_resolved_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_appends_mail_recipients_with_given_type() {
      let val = serde_json::json!([{"id": 111_i64}, {"id": 222_i64}]);
      let vals = val.as_array().unwrap();
      let mut out: Vec<MailRecipient> = Vec::new();

      push_resolved_ids(vals, "corporation", &mut out);

      assert_eq!(out.len(), 2);
      assert_eq!(out[0].recipient_id, 111);
      assert_eq!(out[0].recipient_type, "corporation");
      assert_eq!(out[1].recipient_id, 222);
    }

    #[test]
    fn it_skips_entries_without_id_field() {
      let val = serde_json::json!([{"name": "no-id"}]);
      let vals = val.as_array().unwrap();
      let mut out: Vec<MailRecipient> = Vec::new();

      push_resolved_ids(vals, "character", &mut out);

      assert_eq!(out.len(), 0);
    }
  }

  mod strip_html {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_converts_br_tags_to_newlines() {
      let result = strip_html("Hello<br>World");

      assert_eq!(result, vec!["Hello", "World"]);
    }

    #[test]
    fn it_strips_arbitrary_html_tags() {
      let result = strip_html("<b>Bold</b> text");

      assert_eq!(result, vec!["Bold text"]);
    }

    #[test]
    fn it_decodes_html_entities() {
      let result = strip_html("a &amp; b text");

      assert_eq!(result, vec!["a & b text"]);
    }

    #[test]
    fn it_filters_empty_lines_after_split() {
      let result = strip_html("<br><br>Content");

      assert_eq!(result, vec!["Content"]);
    }
  }

  mod toggle_label_list {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_adds_label_when_not_present() {
      let mut labels = vec![10_i64, 20];

      toggle_label_list(&mut labels, 30, true);

      assert_eq!(labels, vec![10, 20, 30]);
    }

    #[test]
    fn it_does_not_duplicate_existing_label() {
      let mut labels = vec![10_i64, 30];

      toggle_label_list(&mut labels, 30, true);

      assert_eq!(labels, vec![10, 30]);
    }

    #[test]
    fn it_removes_label_when_present() {
      let mut labels = vec![10_i64, 30, 50];

      toggle_label_list(&mut labels, 30, false);

      assert_eq!(labels, vec![10, 50]);
    }

    #[test]
    fn it_is_a_no_op_when_removing_absent_label() {
      let mut labels = vec![10_i64, 20];

      toggle_label_list(&mut labels, 99, false);

      assert_eq!(labels, vec![10, 20]);
    }
  }
}
