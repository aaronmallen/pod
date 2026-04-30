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
  let first_id = characters.first().map(|c| *c.id()).unwrap_or(0);

  let accounts: Vec<MailAccount> = characters
    .iter()
    .map(|c| MailAccount {
      id: *c.id(),
      name: c.name().clone(),
      corp: c.corp_name().clone(),
      tone: *c.portrait_tone() as u16,
      unread: 0,
    })
    .collect();

  let portrait_handles: std::collections::HashMap<i64, image::Handle> = characters
    .iter()
    .filter_map(|c| {
      c.portrait_data()
        .as_ref()
        .map(|b| (*c.id(), image::Handle::from_bytes(b.clone())))
    })
    .collect();

  let picker_entries: Vec<CharacterEntry> = accounts
    .iter()
    .map(|a| CharacterEntry {
      id: Some(a.id),
      name: a.name.clone(),
      corp_name: a.corp.clone(),
      tone: a.tone,
      portrait_handle: portrait_handles.get(&a.id).cloned(),
    })
    .collect();
  let account_picker = CharacterPicker::new()
    .entries(picker_entries.clone())
    .selected(PickerSelection::Character(first_id));

  let compose = pod_ui::components::ComposePanel::new()
    .from_entries(picker_entries)
    .from_selected(Some(first_id));

  let state = State {
    accounts,
    characters: characters.clone(),
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
  };

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
    Message::AccountPicker(msg) => {
      state.account_picker.update(msg);
    }
    Message::FolderPane(folder_pane::Message::FolderSelected(folder)) => {
      state.selected_folder = folder;
      state.selected_message_id = None;
    }
    Message::MessageList(message_list_pane::Message::CursorMoved(x, y)) => {
      state.cursor_pos = (x, y);
    }
    Message::MessageList(message_list_pane::Message::MessageRightClicked(id)) => {
      state.context_menu = Some((id.clone(), state.cursor_pos.0, state.cursor_pos.1));
      state.selected_message_id = Some(id);
      state.snooze_popover_open = false;
    }
    Message::MessageList(message_list_pane::Message::ContextMenuClose) => {
      state.context_menu = None;
    }
    Message::MessageList(message_list_pane::Message::MessageSelected(id)) => {
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
    }
    Message::MessageList(message_list_pane::Message::SearchChanged(q)) => {
      state.search_query = q;
    }
    Message::ReadingPane(reading_pane::Message::ReplyPressed)
    | Message::ReadingPane(reading_pane::Message::ReplyAllPressed) => {
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
      state.compose_open = true;
      state.compose.reset();
      state.compose.to = vec![to];
      state.compose.subject = subject;
      state.compose.from_picker.selected = PickerSelection::Character(msg.character_id);
      state.snooze_popover_open = false;
    }
    Message::ReadingPane(reading_pane::Message::ForwardPressed) => {
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
      state.compose_open = true;
      state.compose.reset();
      state.compose.subject = subject;
      state.compose.body = iced::widget::text_editor::Content::with_text(&body_text);
      state.compose.from_picker.selected = PickerSelection::Character(msg.character_id);
      state.snooze_popover_open = false;
    }
    Message::ReadingPane(reading_pane::Message::StarToggle) => {
      state.context_menu = None;
      if let Some(id) = state.selected_message_id.clone()
        && let Some(msg) = state.messages.iter_mut().find(|m| m.id == id)
      {
        msg.starred = !msg.starred;
      }
    }
    Message::ReadingPane(reading_pane::Message::ArchivePressed) => {
      state.context_menu = None;
      if let Some(id) = state.selected_message_id.clone() {
        if let Some(msg) = state.messages.iter_mut().find(|m| m.id == id) {
          msg.folder = "archive".to_string();
        }
        state.selected_message_id = None;
      }
    }
    Message::ReadingPane(reading_pane::Message::DeletePressed) => {
      state.context_menu = None;
      if let Some(id) = state.selected_message_id.take()
        && let Some(pos) = state.messages.iter().position(|m| m.id == id)
      {
        let mail_id = state.messages[pos].mail_id;
        let character_id = state.messages[pos].character_id;
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
              let _ = db.characters().delete_mail_header(character_id, mail_id).await;
              if let Some(character) = chars.iter().find(|c| *c.id() == character_id)
                && let Some(token) = character_service::ensure_valid_token(character, &esi, &db).await
              {
                let grant = character_service::refresh_grant(character, &token);
                let _ = esi.character(&grant).delete_mail(mail_id).await;
              }
            },
            |_| Message::MailDeleted,
          );
        }
      }
    }
    Message::ComposePressed => {
      let from_id = state.account_picker.selected.clone();
      state.compose_open = true;
      state.compose.reset();
      state.compose.from_picker.selected = from_id;
    }
    Message::Compose(compose_msg) => match &compose_msg {
      compose_panel::Message::Close => {
        state.compose_open = false;
        state.compose.update(compose_msg);
      }
      compose_panel::Message::ToSearchChanged(val) => {
        let val = val.clone();
        state.compose.update(compose_msg);
        if val.trim().len() >= 3
          && let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone())
        {
          let chars = state.characters.clone();
          return iced::Task::perform(async move { search_recipients(val, chars, esi, db).await }, |results| {
            Message::Compose(compose_panel::Message::ToSearchResults(results))
          });
        }
      }
      compose_panel::Message::CcSearchChanged(val) => {
        let val = val.clone();
        state.compose.update(compose_msg);
        if val.trim().len() >= 3
          && let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone())
        {
          let chars = state.characters.clone();
          return iced::Task::perform(async move { search_recipients(val, chars, esi, db).await }, |results| {
            Message::Compose(compose_panel::Message::CcSearchResults(results))
          });
        }
      }
      compose_panel::Message::SendPressed => {
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

        return iced::Task::perform(
          async move { send_composed_mail(from_id, to, cc, subject, body, characters, esi, db).await },
          |result| Message::Compose(compose_panel::Message::Sent(result)),
        );
      }
      compose_panel::Message::Sent(Ok(_)) => {
        state.compose_open = false;
        state.compose.update(compose_msg);
      }
      _ => {
        state.compose.update(compose_msg);
      }
    },
    Message::MailDeleted => {
      recompute_unread_counts(state);
    }
    Message::ReadingPane(reading_pane::Message::SnoozedExpired(pairs)) => {
      for (character_id, mail_id) in pairs {
        let msg_id = format!("{character_id}-{mail_id}");
        if let Some(msg) = state.messages.iter_mut().find(|m| m.id == msg_id) {
          msg.snoozed = None;
          msg.unread = true;
          msg.folder = "inbox".to_string();
        }
      }
    }
    Message::ReadingPane(reading_pane::Message::SnoozeToggle) => {
      state.context_menu = None;
      state.snooze_popover_open = !state.snooze_popover_open;
    }
    Message::ReadingPane(reading_pane::Message::SnoozeFailed(_)) => {}
    Message::ReadingPane(reading_pane::Message::SnoozeSet(label)) => {
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
                let _ = db.characters().upsert_snoozed_mail(character_id, mail_id, until).await;
              }
            } else {
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
    }
    Message::ReadingPane(reading_pane::Message::CheckSnoozed) => {
      if let Some(db) = services.db.clone() {
        let chars = state.characters.clone();
        let esi = services.esi_client.clone();
        return iced::Task::perform(async move { check_expired_snoozes(chars, esi, db).await }, |expired| {
          Message::ReadingPane(reading_pane::Message::SnoozedExpired(expired))
        });
      }
    }
    Message::MailBodyLoaded(msg_id, paragraphs) => {
      if let Some(msg) = state.messages.iter_mut().find(|m| m.id == msg_id) {
        msg.body = paragraphs;
        msg.body_loaded = true;
        msg.unread = false;
      }
    }
    Message::PaneDragStart(pane) => {
      state.dragging_pane = Some(pane);
      state.last_drag_x = 0.0;
    }
    Message::PaneDrag(x) => {
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
    }
    Message::PaneDragEnd => {
      state.dragging_pane = None;
      state.last_drag_x = 0.0;
    }
    Message::MailHeadersLoaded(messages) => {
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
    }
  }
  iced::Task::none()
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
    Err(_) => return Vec::new(),
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

  let mut resolved: Vec<MailRecipient> = Vec::new();
  let mut needs_resolution: Vec<&str> = Vec::new();

  for r in &all_recipients {
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
    let ids_result = esi
      .universe()
      .ids(&needs_resolution)
      .await
      .map_err(|e| format!("Name resolution failed: {e}"))?;

    let push_values = |vals: &[serde_json::Value], rtype: &str, out: &mut Vec<MailRecipient>| {
      for v in vals {
        if let Some(id) = v.get("id").and_then(|x| x.as_i64()) {
          out.push(MailRecipient {
            recipient_id: id,
            recipient_type: rtype.to_string(),
          });
        }
      }
    };

    if let Some(chars) = &ids_result.characters {
      push_values(chars, "character", &mut resolved);
    }
    if let Some(corps) = &ids_result.corporations {
      push_values(corps, "corporation", &mut resolved);
    }
    if let Some(alliances) = &ids_result.alliances {
      push_values(alliances, "alliance", &mut resolved);
    }
  }

  if resolved.is_empty() {
    return Err("No valid recipients found — check character/corporation names".to_string());
  }

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

async fn fetch_mail_headers(characters: Vec<Character>, esi: pod_esi::Client, db: pod_db::Repo) -> Vec<MailMessage> {
  let mut all: Vec<MailMessage> = Vec::new();

  let snoozed_rows = db.characters().all_snoozed_mails().await.unwrap_or_default();
  let snoozed_lookup: std::collections::HashMap<(i64, i64), String> = snoozed_rows
    .into_iter()
    .map(|r| ((r.character_id, r.mail_id), r.snooze_until))
    .collect();

  for character in &characters {
    let Some(token) = character_service::ensure_valid_token(character, &esi, &db).await else {
      continue;
    };
    let grant = character_service::refresh_grant(character, &token);
    let char_client = esi.character(&grant);

    // Fetch ESI headers and build the recipient map.
    let mut recipient_map: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    let esi_headers = match char_client.mail().await {
      Ok(h) => h,
      Err(_) => Vec::new(),
    };
    for h in &esi_headers {
      if let (Some(mail_id), Some(recipients)) = (h.mail_id, &h.recipients) {
        let ids: Vec<i64> = recipients.iter().map(|r| r.recipient_id).collect();
        if !ids.is_empty() {
          recipient_map.insert(mail_id, ids);
        }
      }
    }

    // Collect all IDs we need names for (senders + recipients).
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut all_ids: Vec<i64> = Vec::new();
    for h in &esi_headers {
      if let Some(id) = h.from
        && id != *character.id()
        && seen.insert(id)
      {
        all_ids.push(id);
      }
    }
    for ids in recipient_map.values() {
      for &id in ids {
        if id != *character.id() && seen.insert(id) {
          all_ids.push(id);
        }
      }
    }
    let name_map: std::collections::HashMap<i64, String> = if !all_ids.is_empty() {
      esi
        .universe()
        .names(&all_ids)
        .await
        .map(|ns| ns.into_iter().map(|n| (n.id, n.name)).collect())
        .unwrap_or_default()
    } else {
      std::collections::HashMap::new()
    };

    // Upsert to DB with recipients_display already resolved.
    if !esi_headers.is_empty() {
      let db_rows: Vec<_> = esi_headers
        .iter()
        .filter_map(|h| {
          let mail_id = h.mail_id?;
          let is_sent = h.from == Some(*character.id());
          let recipients_display = if is_sent {
            recipient_map
              .get(&mail_id)
              .map(|ids| {
                ids
                  .iter()
                  .filter_map(|id| name_map.get(id).cloned().or_else(|| Some(format!("#{id}"))))
                  .collect::<Vec<_>>()
                  .join(", ")
              })
              .unwrap_or_default()
          } else {
            String::new()
          };
          Some(MailHeader {
            character_id: *character.id(),
            mail_id,
            subject: h.subject.clone().unwrap_or_default(),
            from_id: h.from,
            is_read: h.is_read.unwrap_or(false),
            timestamp: h.timestamp.clone().unwrap_or_default(),
            recipients_display,
          })
        })
        .collect();
      let _ = db.characters().upsert_mail_headers(*character.id(), &db_rows).await;
    }

    // Load from DB and build MailMessage list.
    if let Ok(rows) = db.characters().mail_headers(*character.id()).await {
      // For any from_id not covered by ESI fetch, supplement name_map from DB rows.
      let mut name_map = name_map;
      let extra_ids: Vec<i64> = rows
        .iter()
        .filter_map(|r| r.from_id)
        .filter(|id| *id != *character.id() && !name_map.contains_key(id))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
      if !extra_ids.is_empty()
        && let Ok(ns) = esi.universe().names(&extra_ids).await
      {
        for n in ns {
          name_map.insert(n.id, n.name);
        }
      }

      for row in rows {
        let is_sent = row.from_id == Some(row.character_id);
        let from_name = if is_sent {
          character.name().clone()
        } else {
          row
            .from_id
            .and_then(|id| name_map.get(&id).cloned())
            .unwrap_or_else(|| row.from_id.map(|id| format!("#{id}")).unwrap_or_default())
        };
        let snooze_key = (row.character_id, row.mail_id);
        let snoozed = snoozed_lookup.get(&snooze_key).cloned();
        let time_label = format_timestamp_label(&row.timestamp);
        let date_label = date_bucket_label(&row.timestamp);
        all.push(MailMessage {
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
          time: time_label,
          date_label,
          unread: !row.is_read && !is_sent,
          starred: false,
          pinned: false,
          has_attachment: false,
          labels: Vec::new(),
          important: false,
          snoozed,
          recipients_display: row.recipients_display,
        });
      }
    }
  }
  all
}

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
    Err(_) => (msg_id, Vec::new()),
  }
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

fn format_timestamp_label(ts: &str) -> String {
  if ts.len() >= 16 {
    ts[11..16].to_string()
  } else {
    ts.to_string()
  }
}

fn date_bucket_label(ts: &str) -> String {
  if ts.len() >= 10 {
    ts[..10].to_string()
  } else {
    ts.to_string()
  }
}

fn snooze_label_to_iso(label: &str) -> Option<String> {
  let now = Utc::now();
  let today = now.date_naive();
  let target = match label {
    "Later today" => today.and_hms_opt(18, 0, 0)?.and_utc(),
    "Tomorrow" => today.succ_opt()?.and_hms_opt(9, 0, 0)?.and_utc(),
    "After downtime" => {
      let downtime = today.and_hms_opt(11, 0, 0)?.and_utc();
      if now >= downtime {
        today.succ_opt()?.and_hms_opt(11, 0, 0)?.and_utc()
      } else {
        downtime
      }
    }
    "Next week" => {
      let days_until_mon = match today.weekday() {
        Weekday::Mon => 7,
        d => (8 - d.number_from_monday() as i64).rem_euclid(7).max(1),
      };
      (today + chrono::Duration::days(days_until_mon))
        .and_hms_opt(9, 0, 0)?
        .and_utc()
    }
    _ => return None,
  };
  Some(target.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

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
    let _ = db.characters().delete_snoozed_mail(row.character_id, row.mail_id).await;
    if let Some(esi) = &esi {
      let chars = characters.clone();
      let _ = apply_snooze_label(row.character_id, row.mail_id, false, chars, esi.clone(), db.clone()).await;
    }
    unsnooze_pairs.push((row.character_id, row.mail_id));
  }
  unsnooze_pairs
}

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
  let char_client = esi.character(&grant);

  let all_labels = char_client
    .mail_labels()
    .await
    .map_err(|e| format!("Failed to fetch labels: {e}"))?;

  let snoozed_id = if let Some(id) = all_labels
    .labels
    .as_deref()
    .unwrap_or_default()
    .iter()
    .find(|l| l.name.as_deref() == Some("Snoozed"))
    .and_then(|l| l.label_id)
  {
    id
  } else if add_label {
    char_client
      .create_mail_label(NewMailLabel {
        color: Some("#ffaa00".to_string()),
        name: "Snoozed".to_string(),
      })
      .await
      .map_err(|e| format!("Failed to create Snoozed label: {e}"))?
  } else {
    return Ok(());
  };

  let mail_body = char_client
    .mail_message(mail_id)
    .await
    .map_err(|e| format!("Failed to fetch mail: {e}"))?;

  let mut mail_labels: Vec<i64> = mail_body.labels.unwrap_or_default();
  if add_label {
    if !mail_labels.contains(&snoozed_id) {
      mail_labels.push(snoozed_id);
    }
  } else {
    mail_labels.retain(|&id| id != snoozed_id);
  }

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
