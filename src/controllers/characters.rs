//! Characters controller: message handling and background subscriptions.

use std::collections::HashMap;

use iced::{
  Event, Subscription,
  keyboard::{self, Key, key::Named},
  mouse,
  widget::image,
};
use pod_model::{Character, CharacterAsset, CharacterSkill, Corporation, TrainingQueueEntry};
use pod_ui::{
  filter_query,
  views::characters::{
    Message, State, characters_tab,
    characters_tab::{character_card, context_menu},
    corporations_tab,
    corporations_tab::{context_menu as corp_context_menu, corporation_card},
    header, search_filter, tag_modal,
  },
};

use crate::services::{Services, character as character_service, corporation as corporation_service};

/// Creates a new characters controller state and a startup task that loads tags per character.
pub fn new(characters: Vec<Character>, services: &Services) -> (State, iced::Task<Message>) {
  let portrait_handles = characters
    .iter()
    .filter_map(|c| {
      c.portrait_data()
        .as_ref()
        .map(|b| (*c.id(), image::Handle::from_bytes(b.clone())))
    })
    .collect();

  let features = services.config.features();
  let mut pane_state = characters_tab::State::new();
  pane_state.portrait_handles = portrait_handles;

  let state = State {
    active_tab: Default::default(),
    add_status: None,
    all_characters: characters.clone(),
    all_corporations: Vec::new(),
    all_tags: Vec::new(),
    character_pane: pane_state,
    characters,
    confirm_remove: None,
    confirm_remove_corporation: None,
    corporation_pane: Default::default(),
    corporations: Vec::new(),
    feat_location_tracking: *features.location_tracking(),
    feat_skill_monitoring: *features.skill_monitoring(),
    feat_wallet: *features.wallet(),
    header: Default::default(),
    search_filter: search_filter::State::new(),
    tag_corpus: Vec::new(),
    tag_modal: None,
  };

  let task = startup_tasks(&state, services);
  (state, task)
}

/// Returns a task that fetches updated character locations from ESI.
pub fn location_refresh_task(state: &State, services: &Services) -> iced::Task<Message> {
  let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone()) else {
    return iced::Task::none();
  };
  let characters = state.all_characters.clone();
  iced::Task::perform(fetch_locations(characters, esi, db), |updates| {
    Message::CharactersTab(characters_tab::Message::LocationsRefreshed(updates))
  })
}

/// Returns a task that fetches updated character skill queues from ESI.
pub fn skill_queue_refresh_task(state: &State, services: &Services) -> iced::Task<Message> {
  let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone()) else {
    return iced::Task::none();
  };
  let characters = state.all_characters.clone();
  iced::Task::perform(fetch_skill_queues(characters, esi, db), |updates| {
    Message::CharactersTab(characters_tab::Message::SkillQueuesRefreshed(updates))
  })
}

/// Returns a task that refreshes character public data (corporation membership) from ESI.
pub fn character_public_refresh_task(state: &State, services: &Services) -> iced::Task<Message> {
  let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone()) else {
    return iced::Task::none();
  };
  let characters = state.all_characters.clone();
  iced::Task::perform(fetch_character_public_data(characters, esi, db), |updates| {
    Message::CharactersTab(characters_tab::Message::CharacterPublicRefreshed(updates))
  })
}

/// Returns a task that refreshes the public corporation data (name, ticker, member count, logo).
pub fn corp_public_refresh_task(state: &State, services: &Services) -> iced::Task<Message> {
  let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone()) else {
    return iced::Task::none();
  };
  let corporations = state.corporations.clone();
  iced::Task::perform(fetch_corp_public_data(corporations, esi, db), |corps| {
    Message::CorporationsTab(corporations_tab::Message::CorpPublicRefreshed(corps))
  })
}

/// Returns a task that refreshes the corporation wallet data.
pub fn corp_wallet_refresh_task(state: &State, services: &Services) -> iced::Task<Message> {
  let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone()) else {
    return iced::Task::none();
  };
  let corporations = state.corporations.clone();
  iced::Task::perform(fetch_corp_wallets(corporations, esi, db), |_| Message::TagsApplied)
}

fn reorder_ids(ids: &[i64], dragging: i64, target: i64) -> Vec<i64> {
  let dragging_pos = ids.iter().position(|&id| id == dragging).unwrap_or(0);
  let target_pos = ids.iter().position(|&id| id == target).unwrap_or(0);
  let mut result = ids.to_vec();
  result.remove(dragging_pos);
  result.insert(target_pos.min(result.len()), dragging);
  result
}

fn reorder_characters_by_ids(characters: &mut Vec<Character>, order: &[i64]) {
  let mut map: std::collections::HashMap<i64, Character> = characters.drain(..).map(|c| (*c.id(), c)).collect();
  for id in order {
    if let Some(c) = map.remove(id) {
      characters.push(c);
    }
  }
}

/// Returns background refresh subscriptions for the characters view.
///
/// Six independent tickers fire at intervals matching ESI cache times:
/// location (60 s), skill queue (120 s), wallet (300 s), corp wallet (300 s),
/// character public/corp (3600 s), corp public (3600 s).
/// Keyboard subscriptions are added based on which overlay is active.
pub fn subscription(state: &State) -> Subscription<Message> {
  let mut subs: Vec<Subscription<Message>> = ticker_subscriptions(state);

  if state.character_pane.dragging_id.is_some() {
    subs.push(drag_release_subscription());
  }

  subs.push(active_keyboard_subscription(state));

  Subscription::batch(subs)
}

/// Returns a task that fetches updated character wallet balances from ESI.
pub fn wallet_refresh_task(state: &State, services: &Services) -> iced::Task<Message> {
  let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone()) else {
    return iced::Task::none();
  };
  let characters = state.all_characters.clone();
  iced::Task::perform(fetch_wallets(characters, esi, db), |updates| {
    Message::CharactersTab(characters_tab::Message::WalletsRefreshed(updates))
  })
}

/// Processes a characters message and returns a task.
pub fn update(state: &mut State, message: Message, services: &Services) -> iced::Task<Message> {
  match message {
    Message::AddCharacterError(e) => {
      state.add_status = Some(format!("Error: {e}"));
      iced::Task::none()
    }
    Message::AddCorporationError(e) => {
      state.add_status = Some(format!("Error: {e}"));
      iced::Task::none()
    }
    Message::AllTagsLoaded(tags) => {
      state.all_tags = tags;
      iced::Task::none()
    }
    Message::CharactersTab(msg) => update_characters_tab(state, msg, services),
    Message::ConfirmRemove => update_confirm_remove(state, services),
    Message::ConfirmRemoveCorporation => update_confirm_remove_corporation(state, services),
    Message::CorporationsTab(msg) => update_corporation(state, msg, services),
    Message::DismissConfirmRemove => {
      state.confirm_remove = None;
      iced::Task::none()
    }
    Message::DismissConfirmRemoveCorporation => {
      state.confirm_remove_corporation = None;
      iced::Task::none()
    }
    Message::Header(msg) => update_header(state, msg, services),
    Message::SearchFilter(msg) => update_search_filter(state, msg),
    Message::TagModal(msg) => handle_tag_modal(state, msg, services),
    Message::TagsApplied => iced::Task::none(),
  }
}

fn refilter(state: &mut State) {
  let q = state.search_filter.query.clone();
  state.characters = filter_characters(&state.all_characters, &q);
  state.corporations = filter_corporations(&state.all_corporations, &q);
}

fn recompute_tag_corpus(state: &mut State) {
  if let Some(modal) = &state.tag_modal {
    if modal.entity_type == "corporation" {
      state.tag_corpus = build_corp_tag_corpus(&state.all_corporations);
    } else {
      state.tag_corpus = build_tag_corpus(&state.all_characters);
    }
  } else {
    state.tag_corpus = Vec::new();
  }
}

fn build_tag_corpus(characters: &[Character]) -> Vec<(String, usize)> {
  use std::collections::HashMap;
  let mut counts: HashMap<String, usize> = HashMap::new();
  for c in characters {
    for (_, name) in c.tags() {
      *counts.entry(name.clone()).or_default() += 1;
    }
  }
  let mut corpus: Vec<(String, usize)> = counts.into_iter().collect();
  corpus.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
  corpus
}

fn build_corp_tag_corpus(corporations: &[Corporation]) -> Vec<(String, usize)> {
  use std::collections::HashMap;
  let mut counts: HashMap<String, usize> = HashMap::new();
  for c in corporations {
    for (_, name) in c.tags() {
      *counts.entry(name.clone()).or_default() += 1;
    }
  }
  let mut corpus: Vec<(String, usize)> = counts.into_iter().collect();
  corpus.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
  corpus
}

fn filter_corporations(corporations: &[Corporation], query: &str) -> Vec<Corporation> {
  if query.trim().is_empty() {
    return corporations.to_vec();
  }
  let q = query.trim().to_lowercase();
  corporations
    .iter()
    .filter(|c| {
      c.name().to_lowercase().contains(&q)
        || c.ticker().to_lowercase().contains(&q)
        || c
          .alliance_name()
          .as_ref()
          .map(|a| a.to_lowercase().contains(&q))
          .unwrap_or(false)
    })
    .cloned()
    .collect()
}

fn filter_characters(characters: &[Character], query: &str) -> Vec<Character> {
  if query.trim().is_empty() {
    return characters.to_vec();
  }
  let parsed = filter_query::parse(query);
  if parsed.tokens.is_empty() {
    return characters.to_vec();
  }
  characters
    .iter()
    .filter(|c| parsed.matches_character(c))
    .cloned()
    .collect()
}

fn confirm_remove_keyboard_subscription() -> Subscription<Message> {
  iced::event::listen_with(|event, _status, _id| {
    let Event::Keyboard(keyboard::Event::KeyPressed {
      key, ..
    }) = event
    else {
      return None;
    };
    match key {
      Key::Named(Named::Enter) => Some(Message::ConfirmRemove),
      Key::Named(Named::Escape) => Some(Message::DismissConfirmRemove),
      _ => None,
    }
  })
}

fn confirm_remove_corporation_keyboard_subscription() -> Subscription<Message> {
  iced::event::listen_with(|event, _status, _id| {
    let Event::Keyboard(keyboard::Event::KeyPressed {
      key, ..
    }) = event
    else {
      return None;
    };
    match key {
      Key::Named(Named::Enter) => Some(Message::ConfirmRemoveCorporation),
      Key::Named(Named::Escape) => Some(Message::DismissConfirmRemoveCorporation),
      _ => None,
    }
  })
}

fn context_menu_keyboard_subscription() -> Subscription<Message> {
  iced::event::listen_with(|event, status, _id| match event {
    Event::Keyboard(keyboard::Event::KeyPressed {
      key: Key::Named(Named::Escape),
      ..
    }) => Some(Message::CharactersTab(characters_tab::Message::ContextMenu(
      context_menu::Message::Close,
    ))),
    Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if status == iced::event::Status::Ignored => Some(
      Message::CharactersTab(characters_tab::Message::ContextMenu(context_menu::Message::Close)),
    ),
    _ => None,
  })
}

fn corp_context_menu_keyboard_subscription() -> Subscription<Message> {
  iced::event::listen_with(|event, status, _id| match event {
    Event::Keyboard(keyboard::Event::KeyPressed {
      key: Key::Named(Named::Escape),
      ..
    }) => Some(Message::CorporationsTab(corporations_tab::Message::ContextMenu(
      corp_context_menu::Message::Close,
    ))),
    Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if status == iced::event::Status::Ignored => {
      Some(Message::CorporationsTab(corporations_tab::Message::ContextMenu(
        corp_context_menu::Message::Close,
      )))
    }
    _ => None,
  })
}

fn handle_tag_modal(state: &mut State, msg: tag_modal::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    tag_modal::Message::Close => {
      state.tag_modal = None;
      state.tag_corpus = Vec::new();
      iced::Task::none()
    }
    tag_modal::Message::QueryChanged(q) => {
      if let Some(m) = &mut state.tag_modal {
        m.query = q;
        m.highlighted = 0;
      }
      iced::Task::none()
    }
    tag_modal::Message::Highlighted(i) => {
      if let Some(m) = &mut state.tag_modal {
        m.highlighted = i;
      }
      iced::Task::none()
    }
    tag_modal::Message::MoveUp => {
      if let Some(m) = &mut state.tag_modal {
        m.highlighted = m.highlighted.saturating_sub(1);
      }
      iced::Task::none()
    }
    tag_modal::Message::MoveDown => {
      if let Some(m) = &mut state.tag_modal {
        let count = tag_modal::compute_items(m, &state.tag_corpus).len();
        if count > 0 && m.highlighted < count - 1 {
          m.highlighted += 1;
        }
      }
      iced::Task::none()
    }
    tag_modal::Message::CommitHighlighted => handle_tag_commit_highlighted(state, services),
    tag_modal::Message::Confirm(name) => handle_tag_confirm(state, name, services),
    tag_modal::Message::Remove(tag_id) => handle_tag_remove(state, tag_id, services),
  }
}

fn handle_tag_commit_highlighted(state: &mut State, services: &Services) -> iced::Task<Message> {
  let Some(m) = &state.tag_modal else {
    return iced::Task::none();
  };
  let corpus = state.tag_corpus.clone();
  let items = tag_modal::compute_items(m, &corpus);
  let hi = m.highlighted.min(items.len().saturating_sub(1));
  let Some(item) = items.into_iter().nth(hi) else {
    return iced::Task::none();
  };
  let tag_name = item.name;
  let entity_id = m.entity_id;
  let entity_type = m.entity_type.clone();
  let existing_ids: Vec<i32> = m.existing_tags.iter().map(|(id, _)| *id).collect();
  state.tag_modal = None;
  apply_tag_task(entity_id, entity_type, tag_name, existing_ids, services)
}

fn handle_tag_confirm(state: &mut State, name: String, services: &Services) -> iced::Task<Message> {
  let Some(m) = &state.tag_modal else {
    return iced::Task::none();
  };
  let entity_id = m.entity_id;
  let entity_type = m.entity_type.clone();
  let existing_ids: Vec<i32> = m.existing_tags.iter().map(|(id, _)| *id).collect();
  state.tag_modal = None;
  apply_tag_task(entity_id, entity_type, name, existing_ids, services)
}

fn handle_tag_remove(state: &mut State, tag_id: i32, services: &Services) -> iced::Task<Message> {
  let Some(m) = &mut state.tag_modal else {
    return iced::Task::none();
  };
  m.existing_tags.retain(|(id, _)| *id != tag_id);
  let entity_id = m.entity_id;
  let entity_type = m.entity_type.clone();
  let new_ids: Vec<i32> = m.existing_tags.iter().map(|(id, _)| *id).collect();
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  if entity_type == "corporation" {
    iced::Task::perform(
      async move {
        db.tags().set_corporation_tags(entity_id, new_ids).await.ok()?;
        let tags = db.tags().tags_for_corporation(entity_id).await.ok()?;
        Some((entity_id, tags.into_iter().map(|t| (t.id, t.name)).collect::<Vec<_>>()))
      },
      |result| match result {
        Some((id, tags)) => Message::CorporationsTab(corporations_tab::Message::CorporationTagsLoaded(id, tags)),
        None => Message::TagsApplied,
      },
    )
  } else {
    iced::Task::perform(
      async move {
        db.tags().set_character_tags(entity_id, new_ids).await.ok()?;
        let tags = db.tags().tags_for_character(entity_id).await.ok()?;
        Some((entity_id, tags.into_iter().map(|t| (t.id, t.name)).collect::<Vec<_>>()))
      },
      |result| match result {
        Some((id, tags)) => Message::CharactersTab(characters_tab::Message::CharacterTagsLoaded(id, tags)),
        None => Message::TagsApplied,
      },
    )
  }
}

fn apply_tag_task(
  entity_id: i64,
  entity_type: String,
  tag_name: String,
  existing_ids: Vec<i32>,
  services: &Services,
) -> iced::Task<Message> {
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  if entity_type == "corporation" {
    iced::Task::perform(
      async move {
        let tag = db.tags().find_or_create(&tag_name).await.ok()?;
        let mut new_ids = existing_ids;
        if !new_ids.contains(&tag.id) {
          new_ids.push(tag.id);
        }
        db.tags().set_corporation_tags(entity_id, new_ids).await.ok()?;
        let tags = db.tags().tags_for_corporation(entity_id).await.ok()?;
        Some((entity_id, tags.into_iter().map(|t| (t.id, t.name)).collect::<Vec<_>>()))
      },
      |result| match result {
        Some((id, tags)) => Message::CorporationsTab(corporations_tab::Message::CorporationTagsLoaded(id, tags)),
        None => Message::TagsApplied,
      },
    )
  } else {
    iced::Task::perform(
      async move {
        let tag = db.tags().find_or_create(&tag_name).await.ok()?;
        let mut new_ids = existing_ids;
        if !new_ids.contains(&tag.id) {
          new_ids.push(tag.id);
        }
        db.tags().set_character_tags(entity_id, new_ids).await.ok()?;
        let tags = db.tags().tags_for_character(entity_id).await.ok()?;
        Some((entity_id, tags.into_iter().map(|t| (t.id, t.name)).collect::<Vec<_>>()))
      },
      |result| match result {
        Some((id, tags)) => Message::CharactersTab(characters_tab::Message::CharacterTagsLoaded(id, tags)),
        None => Message::TagsApplied,
      },
    )
  }
}

fn drag_release_subscription() -> Subscription<Message> {
  iced::event::listen_with(|event, _status, _id| {
    if let Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
      Some(Message::CharactersTab(characters_tab::Message::DragEnd))
    } else {
      None
    }
  })
}

fn tag_modal_keyboard_subscription() -> Subscription<Message> {
  iced::event::listen_with(|event, _status, _id| {
    let Event::Keyboard(keyboard::Event::KeyPressed {
      key, ..
    }) = event
    else {
      return None;
    };
    match key {
      Key::Named(Named::Escape) => Some(Message::TagModal(tag_modal::Message::Close)),
      Key::Named(Named::ArrowUp) => Some(Message::TagModal(tag_modal::Message::MoveUp)),
      Key::Named(Named::ArrowDown) => Some(Message::TagModal(tag_modal::Message::MoveDown)),
      Key::Named(Named::Enter) => Some(Message::TagModal(tag_modal::Message::CommitHighlighted)),
      _ => None,
    }
  })
}

fn filter_keyboard_subscription() -> Subscription<Message> {
  iced::event::listen_with(|event, _status, _id| {
    let Event::Keyboard(keyboard::Event::KeyPressed {
      key,
      modifiers,
      ..
    }) = event
    else {
      return None;
    };
    match key {
      Key::Character(c) if c.as_ref() == "k" && (modifiers.command() || modifiers.control()) => {
        Some(Message::SearchFilter(search_filter::Message::FocusInput))
      }
      Key::Named(Named::Escape) => Some(Message::SearchFilter(search_filter::Message::QueryChanged(
        String::new(),
      ))),
      _ => None,
    }
  })
}

/// Returns the keyboard subscription matching the active overlay or default filter.
fn active_keyboard_subscription(state: &State) -> Subscription<Message> {
  if state.tag_modal.is_some() {
    tag_modal_keyboard_subscription()
  } else if state.character_pane.context_menu.is_some() {
    context_menu_keyboard_subscription()
  } else if state.corporation_pane.context_menu.is_some() {
    corp_context_menu_keyboard_subscription()
  } else if state.confirm_remove.is_some() {
    confirm_remove_keyboard_subscription()
  } else if state.confirm_remove_corporation.is_some() {
    confirm_remove_corporation_keyboard_subscription()
  } else {
    filter_keyboard_subscription()
  }
}

/// Returns timer-based refresh subscriptions.
fn ticker_subscriptions(state: &State) -> Vec<Subscription<Message>> {
  let mut subs: Vec<Subscription<Message>> = Vec::new();

  if state.feat_location_tracking {
    subs.push(
      iced::time::every(std::time::Duration::from_secs(60))
        .map(|_| Message::CharactersTab(characters_tab::Message::LocationRefreshTick)),
    );
  }
  if state.feat_skill_monitoring {
    subs.push(
      iced::time::every(std::time::Duration::from_secs(120))
        .map(|_| Message::CharactersTab(characters_tab::Message::SkillQueueRefreshTick)),
    );
  }
  if state.feat_wallet {
    subs.push(
      iced::time::every(std::time::Duration::from_secs(300))
        .map(|_| Message::CharactersTab(characters_tab::Message::WalletRefreshTick)),
    );
  }
  subs.extend([
    iced::time::every(std::time::Duration::from_secs(300))
      .map(|_| Message::CorporationsTab(corporations_tab::Message::CorpWalletRefreshTick)),
    iced::time::every(std::time::Duration::from_secs(3600))
      .map(|_| Message::CharactersTab(characters_tab::Message::CharacterPublicRefreshTick)),
    iced::time::every(std::time::Duration::from_secs(3600))
      .map(|_| Message::CorporationsTab(corporations_tab::Message::CorpPublicRefreshTick)),
  ]);
  subs
}

/// Builds the startup batch task that pre-loads tags, corporations, and public data.
fn startup_tasks(state: &State, services: &Services) -> iced::Task<Message> {
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };

  let char_ids: Vec<i64> = state.all_characters.iter().map(|c| *c.id()).collect();

  let all_tags_task = {
    let db = db.clone();
    iced::Task::perform(
      async move {
        db.tags()
          .find_all()
          .await
          .unwrap_or_default()
          .into_iter()
          .map(|t| (t.id, t.name))
          .collect()
      },
      Message::AllTagsLoaded,
    )
  };

  let char_tasks = startup_char_tag_tasks(char_ids, db.clone());

  let corps_task = {
    let db = db.clone();
    iced::Task::perform(
      async move { db.corporations().all().await.unwrap_or_default() },
      |corps| Message::CorporationsTab(corporations_tab::Message::CorporationsLoaded(corps)),
    )
  };

  let pub_refresh_task = if let (Some(esi), Some(db)) = (services.esi_client.clone(), services.db.clone()) {
    let chars = state.all_characters.clone();
    iced::Task::perform(fetch_character_public_data(chars, esi, db), |updates| {
      Message::CharactersTab(characters_tab::Message::CharacterPublicRefreshed(updates))
    })
  } else {
    iced::Task::none()
  };

  iced::Task::batch([all_tags_task, char_tasks, corps_task, pub_refresh_task])
}

/// Builds per-character tag-loading tasks for startup.
fn startup_char_tag_tasks(char_ids: Vec<i64>, db: pod_db::Repo) -> iced::Task<Message> {
  iced::Task::batch(char_ids.into_iter().map(|char_id| {
    let db = db.clone();
    iced::Task::perform(
      async move {
        let tags = db
          .tags()
          .tags_for_character(char_id)
          .await
          .unwrap_or_default()
          .into_iter()
          .map(|t| (t.id, t.name))
          .collect();
        (char_id, tags)
      },
      |(id, tags)| Message::CharactersTab(characters_tab::Message::CharacterTagsLoaded(id, tags)),
    )
  }))
}

async fn fetch_character_public_data(
  characters: Vec<Character>,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Vec<(i64, i64, String)> {
  let mut results = Vec::new();
  for character in &characters {
    let Ok(detail) = esi.character_public(*character.id()).detail().await else {
      continue;
    };
    let corp_id = detail.corporation_id;
    let corp_name = if corp_id > 0 {
      esi
        .corporation(corp_id)
        .detail()
        .await
        .ok()
        .map(|d| d.name)
        .unwrap_or_default()
    } else {
      String::new()
    };
    let _ = db
      .characters()
      .update_corp(*character.id(), corp_id, corp_name.clone())
      .await;
    results.push((*character.id(), corp_id, corp_name));
  }
  results
}

async fn fetch_locations(
  characters: Vec<Character>,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Vec<(i64, Option<String>, Option<bool>)> {
  let mut results = Vec::new();
  for character in &characters {
    let Some(token) = character_service::ensure_valid_token(character, &esi, &db).await else {
      continue;
    };
    let grant = character_service::refresh_grant(character, &token);
    let char_client = esi.character(&grant);
    let Ok(loc) = char_client.location().await else {
      continue;
    };
    let docked = loc.station_id.is_some() || loc.structure_id.is_some();
    let name = if let Some(sid) = loc.station_id {
      esi.universe().station(sid).await.ok().map(|s| s.name)
    } else {
      esi
        .universe()
        .solar_system(loc.solar_system_id)
        .await
        .ok()
        .map(|s| s.name)
    };
    let _ = db
      .characters()
      .update_location(*character.id(), name.clone(), Some(docked))
      .await;
    results.push((*character.id(), name, Some(docked)));
  }
  results
}

async fn fetch_skill_queues(
  characters: Vec<Character>,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Vec<(i64, Vec<CharacterSkill>, Vec<TrainingQueueEntry>)> {
  let mut results = Vec::new();
  for character in &characters {
    let Some(token) = character_service::ensure_valid_token(character, &esi, &db).await else {
      continue;
    };
    let grant = character_service::refresh_grant(character, &token);
    let char_client = esi.character(&grant);
    let Ok(queue) = char_client.skill_queue().await else {
      continue;
    };
    let mut updated_skills = character_service::reconcile_skills(*character.id(), character.skills(), queue.clone());
    let type_ids: Vec<i32> = updated_skills.iter().map(|s| s.skill_id).collect();
    if let Ok(types) = db.universe().item_types().find_by_ids(&type_ids).await {
      let name_map: HashMap<i32, String> = types.into_iter().map(|t| (t.id, t.name)).collect();
      for skill in &mut updated_skills {
        if let Some(name) = name_map.get(&skill.skill_id) {
          skill.skill_name = Some(name.clone());
        }
      }
    }
    let training_queue = character_service::build_training_queue(&queue, &updated_skills);
    let _ = db.characters().upsert_skills(*character.id(), &updated_skills).await;
    results.push((*character.id(), updated_skills, training_queue));
  }
  results
}

async fn fetch_wallets(characters: Vec<Character>, esi: pod_esi::Client, db: pod_db::Repo) -> Vec<(i64, Option<f64>)> {
  let mut results = Vec::new();
  for character in &characters {
    let Some(token) = character_service::ensure_valid_token(character, &esi, &db).await else {
      continue;
    };
    let grant = character_service::refresh_grant(character, &token);
    let char_client = esi.character(&grant);
    let Ok(balance) = char_client.wallet_balance().await else {
      continue;
    };
    let isk = Some(balance.0);
    let _ = db.characters().update_wallet(*character.id(), isk).await;
    results.push((*character.id(), isk));
  }
  results
}

/// Resolves the corporation name for a given corp ID via ESI.
async fn resolve_corp_name(corp_id: i64, esi: &pod_esi::Client) -> String {
  if corp_id > 0 {
    esi
      .corporation(corp_id)
      .detail()
      .await
      .ok()
      .map(|d| d.name)
      .unwrap_or_default()
  } else {
    String::new()
  }
}

/// Resolves the alliance name for an optional alliance ID via ESI.
async fn resolve_alliance_name(alliance_id: Option<i64>, esi: &pod_esi::Client) -> Option<String> {
  let id = alliance_id?;
  esi.alliance(id).detail().await.ok().map(|a| a.name)
}

/// Resolves the HQ station name for an optional station ID via ESI.
async fn resolve_hq_name(station_id: Option<i64>, esi: &pod_esi::Client) -> Option<String> {
  let sid = station_id?;
  esi.universe().station(sid).await.ok().map(|s| s.name)
}

async fn add_character(
  esi: pod_esi::Client,
  verifier: String,
  oauth_state: String,
  db: Option<pod_db::Repo>,
) -> Result<Character, String> {
  let grant = exchange_oauth_grant(&esi, verifier, oauth_state).await?;

  let character_id = *grant.character_id();
  let char_detail = esi.character_public(character_id).detail().await.ok();
  let corp_id = char_detail.as_ref().map(|d| d.corporation_id).unwrap_or(0);
  let character_name = char_detail
    .as_ref()
    .map(|d| d.name.clone())
    .unwrap_or_else(|| grant.character_name().clone());
  let corp_name = resolve_corp_name(corp_id, &esi).await;

  let (mut character, character_skills, character_assets) =
    build_character_from_grant(&grant, character_id, character_name, corp_id, corp_name, &esi).await;

  if let Some(db) = db {
    persist_character(&db, &mut character, &character_skills, &character_assets).await?;
  }
  Ok(character)
}

/// Assembles a `Character` and its initial skills and assets from an OAuth grant and ESI data.
async fn build_character_from_grant(
  grant: &pod_esi::models::auth::Grant,
  character_id: i64,
  character_name: String,
  corp_id: i64,
  corp_name: String,
  esi: &pod_esi::Client,
) -> (Character, Vec<CharacterSkill>, Vec<CharacterAsset>) {
  let access_token = grant.access_token().clone();
  let expires_at = grant
    .expires_at()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);
  let granted_scopes = if grant.scopes().is_empty() {
    None
  } else {
    Some(grant.scopes().join(" "))
  };
  let portrait_tone = (character_id % 360) as i32;
  let refresh_token = grant.refresh_token().clone();

  let (character_skills, tq, portrait_data, isk_balance, location_name, location_docked, raw_assets, attrs) =
    fetch_character_esi_data(character_id, grant, esi).await;

  let mut character = Character::new(character_id, character_name);
  character
    .set_access_token(access_token)
    .set_corp_id(corp_id)
    .set_corp_name(corp_name)
    .set_granted_scopes(granted_scopes)
    .set_isk_balance(isk_balance)
    .set_location_docked(location_docked)
    .set_location_name(location_name)
    .set_portrait_tone(portrait_tone)
    .set_refresh_token(refresh_token)
    .set_token_expires_at(expires_at);
  *character.portrait_data_mut() = portrait_data;
  *character.skills_mut() = character_skills.clone();
  *character.training_queue_mut() = tq;
  apply_attributes_from_esi(&mut character, attrs);

  let character_assets = map_character_assets(character_id, raw_assets);
  (character, character_skills, character_assets)
}

/// Fetches all ESI data for a character in parallel and returns processed results.
#[allow(clippy::type_complexity)]
async fn fetch_character_esi_data(
  character_id: i64,
  grant: &pod_esi::models::auth::Grant,
  esi: &pod_esi::Client,
) -> (
  Vec<CharacterSkill>,
  Vec<TrainingQueueEntry>,
  Option<Vec<u8>>,
  Option<f64>,
  Option<String>,
  Option<bool>,
  Vec<pod_esi::models::character::Asset>,
  Option<pod_esi::models::character::CharacterAttributes>,
) {
  let char_client = esi.character(grant);
  let (skills_result, queue_result, wallet_result, location_result, assets_result, attrs_result) = tokio::join!(
    char_client.skills(),
    char_client.skill_queue(),
    char_client.wallet_balance(),
    char_client.location(),
    char_client.assets(),
    char_client.attributes(),
  );

  let queue_data = queue_result.ok().unwrap_or_default();
  let isk_balance = wallet_result.ok().map(|w| w.0);
  let (location_name, location_docked) = resolve_location_result(location_result, esi).await;
  let raw_assets = assets_result.ok().unwrap_or_default();

  let character_skills = character_service::build_character_skills(
    character_id,
    skills_result.ok().map(|sd| sd.skills).unwrap_or_default(),
    queue_data.clone(),
  );
  let character_skills = character_service::inject_skill_names(character_skills, esi).await;
  let tq = character_service::build_training_queue(&queue_data, &character_skills);
  let portrait_data = character_service::fetch_portrait(character_id, esi).await;

  (
    character_skills,
    tq,
    portrait_data,
    isk_balance,
    location_name,
    location_docked,
    raw_assets,
    attrs_result.ok(),
  )
}

/// Resolves location name and docked state from a location result.
async fn resolve_location_result(
  result: Result<pod_esi::models::character::CharacterLocation, pod_esi::Error>,
  esi: &pod_esi::Client,
) -> (Option<String>, Option<bool>) {
  let Ok(loc) = result else {
    return (None, None);
  };
  let docked = loc.station_id.is_some() || loc.structure_id.is_some();
  let name = if let Some(sid) = loc.station_id {
    esi.universe().station(sid).await.ok().map(|s| s.name)
  } else {
    esi
      .universe()
      .solar_system(loc.solar_system_id)
      .await
      .ok()
      .map(|s| s.name)
  };
  (name, Some(docked))
}

/// Applies ESI attribute data to a character if available.
fn apply_attributes_from_esi(
  character: &mut Character,
  attrs: Option<pod_esi::models::character::CharacterAttributes>,
) {
  if let Some(esi_attrs) = attrs {
    use pod_model::CharacterAttributes;
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
}

/// Maps raw ESI asset records to the model type.
fn map_character_assets(character_id: i64, raw: Vec<pod_esi::models::character::Asset>) -> Vec<CharacterAsset> {
  raw
    .into_iter()
    .map(|a| CharacterAsset {
      item_id: a.item_id,
      character_id,
      type_id: a.type_id,
      location_id: a.location_id,
      location_type: a.location_type,
      location_flag: a.location_flag,
      quantity: a.quantity,
      is_singleton: a.is_singleton,
      is_blueprint_copy: a.is_blueprint_copy,
    })
    .collect()
}

/// Persists a character and its skills and assets to the database.
///
/// Assigns a slot index one past the current maximum so new characters land at the end of the
/// grid without colliding with existing ones. The upsert's ON CONFLICT clause does not update
/// sort_order, so re-authorizing an existing character preserves its current slot.
async fn persist_character(
  db: &pod_db::Repo,
  character: &mut Character,
  skills: &[CharacterSkill],
  assets: &[CharacterAsset],
) -> Result<(), String> {
  let character_id = *character.id();
  let next_order = db.characters().next_sort_order().await.map_err(|e| e.to_string())?;
  character.set_sort_order(next_order);
  db.characters().upsert(character).await.map_err(|e| e.to_string())?;
  db.characters()
    .upsert_skills(character_id, skills)
    .await
    .map_err(|e| e.to_string())?;
  db.characters()
    .upsert_assets(character_id, assets)
    .await
    .map_err(|e| e.to_string())?;
  Ok(())
}

/// Runs the OAuth flow for re-authorizing an existing character with updated scopes.
///
/// Behaves identically to `add_character`; the upsert in the DB ensures the
/// existing record is updated rather than a duplicate being created.
pub async fn reauthorize_character(
  esi: pod_esi::Client,
  verifier: String,
  oauth_state: String,
  db: Option<pod_db::Repo>,
) -> Result<Character, String> {
  add_character(esi, verifier, oauth_state, db).await
}

async fn add_corporation(
  esi: pod_esi::Client,
  verifier: String,
  oauth_state: String,
  db: Option<pod_db::Repo>,
) -> Result<Corporation, String> {
  let grant = exchange_oauth_grant(&esi, verifier, oauth_state).await?;

  let auth_character_id = *grant.character_id();
  let access_token = grant.access_token().clone();
  let expires_at = grant
    .expires_at()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);
  let refresh_token = grant.refresh_token().clone();
  let scopes = grant.scopes().clone();

  let char_detail = esi
    .character_public(auth_character_id)
    .detail()
    .await
    .map_err(|e| e.to_string())?;
  let corp_id = char_detail.corporation_id;
  let detail = esi.corporation(corp_id).detail().await.map_err(|e| e.to_string())?;
  let icon_data = esi.images().corporation_logo(corp_id, 128).await.ok();
  let alliance_name = resolve_alliance_name(detail.alliance_id, &esi).await;
  let hq_name = resolve_hq_name(detail.home_station_id, &esi).await;

  let corp = build_corporation(
    corp_id,
    detail,
    auth_character_id,
    access_token,
    refresh_token,
    expires_at,
    scopes,
    alliance_name,
    hq_name,
    icon_data,
  );

  if let Some(db) = db {
    db.corporations().upsert(&corp).await.map_err(|e| e.to_string())?;
  }
  Ok(corp)
}

/// Performs the OAuth callback/exchange flow and returns the resulting grant.
async fn exchange_oauth_grant(
  esi: &pod_esi::Client,
  verifier: String,
  oauth_state: String,
) -> Result<pod_esi::models::auth::Grant, String> {
  let (code, returned_state) = esi.auth().await_callback(47823).await.map_err(|e| e.to_string())?;
  esi
    .auth()
    .validate_state(&oauth_state, &returned_state)
    .map_err(|e| e.to_string())?;
  esi
    .auth()
    .exchange_code(&code, "http://127.0.0.1:47823/callback", &verifier)
    .await
    .map_err(|e| e.to_string())
}

/// Assembles a `Corporation` from grant data and ESI detail.
#[allow(clippy::too_many_arguments)]
fn build_corporation(
  corp_id: i64,
  detail: pod_esi::models::corporation::CorporationDetail,
  auth_character_id: i64,
  access_token: String,
  refresh_token: String,
  expires_at: i64,
  scopes: Vec<String>,
  alliance_name: Option<String>,
  hq_name: Option<String>,
  icon_data: Option<Vec<u8>>,
) -> Corporation {
  let mut corp = Corporation::new(corp_id, detail.name);
  corp
    .set_access_token(access_token)
    .set_alliance_id(detail.alliance_id)
    .set_alliance_name(alliance_name)
    .set_auth_character_id(auth_character_id)
    .set_ceo_character_id(detail.ceo_id)
    .set_date_founded(detail.date_founded)
    .set_description(detail.description)
    .set_faction_id(detail.faction_id)
    .set_home_station_id(detail.home_station_id)
    .set_member_count(detail.member_count)
    .set_refresh_token(refresh_token)
    .set_shares(detail.shares)
    .set_tax_rate(detail.tax_rate)
    .set_ticker(detail.ticker)
    .set_token_expires_at(expires_at)
    .set_url(detail.url)
    .set_war_eligible(detail.war_eligible);
  *corp.hq_name_mut() = hq_name;
  *corp.icon_data_mut() = icon_data;
  *corp.scopes_mut() = scopes;
  corp
}

async fn fetch_corp_public_data(
  corporations: Vec<Corporation>,
  esi: pod_esi::Client,
  db: pod_db::Repo,
) -> Vec<Corporation> {
  let mut updated = Vec::new();
  for corp in &corporations {
    let Ok(detail) = esi.corporation(*corp.id()).detail().await else {
      continue;
    };
    let icon_data = esi
      .images()
      .corporation_logo(*corp.id(), 128)
      .await
      .ok()
      .or_else(|| corp.icon_data().clone());
    let Some(token) = corporation_service::ensure_valid_token(corp, &esi, &db).await else {
      continue;
    };
    let alliance_name = resolve_alliance_name(detail.alliance_id, &esi).await;
    let hq_name = resolve_hq_name(detail.home_station_id, &esi).await;
    let refreshed = apply_corp_public_refresh(corp, token, detail, alliance_name, hq_name, icon_data);
    let _ = db.corporations().upsert(&refreshed).await;
    updated.push(refreshed);
  }
  updated
}

/// Clones a corporation and applies refreshed ESI public-data fields to it.
fn apply_corp_public_refresh(
  corp: &Corporation,
  token: String,
  detail: pod_esi::models::corporation::CorporationDetail,
  alliance_name: Option<String>,
  hq_name: Option<String>,
  icon_data: Option<Vec<u8>>,
) -> Corporation {
  let mut refreshed = corp.clone();
  refreshed
    .set_access_token(token)
    .set_alliance_id(detail.alliance_id)
    .set_alliance_name(alliance_name)
    .set_ceo_character_id(detail.ceo_id)
    .set_date_founded(detail.date_founded)
    .set_description(detail.description)
    .set_faction_id(detail.faction_id)
    .set_home_station_id(detail.home_station_id)
    .set_member_count(detail.member_count)
    .set_name(detail.name)
    .set_shares(detail.shares)
    .set_tax_rate(detail.tax_rate)
    .set_ticker(detail.ticker)
    .set_url(detail.url)
    .set_war_eligible(detail.war_eligible);
  *refreshed.hq_name_mut() = hq_name;
  *refreshed.icon_data_mut() = icon_data;
  refreshed
}

async fn fetch_corp_wallets(corporations: Vec<Corporation>, esi: pod_esi::Client, db: pod_db::Repo) {
  for corp in &corporations {
    let Some(token) = corporation_service::ensure_valid_token(corp, &esi, &db).await else {
      continue;
    };
    let grant = corporation_service::refresh_grant(corp, &token);
    let _ = esi.corporation(*corp.id()).auth(&grant).wallets().await;
  }
}

fn update_character_card(state: &mut State, msg: characters_tab::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    characters_tab::Message::Card(char_id, character_card::Message::NamePressed(id)) => {
      let _ = state
        .character_pane
        .update(characters_tab::Message::Card(
          char_id,
          character_card::Message::NamePressed(id),
        ))
        .map(Message::CharactersTab);
      iced::Task::done(Message::CharactersTab(characters_tab::Message::NavigateToDetail(id)))
    }
    characters_tab::Message::Card(char_id, character_card::Message::SkillTrainingPressed(id)) => {
      let _ = state
        .character_pane
        .update(characters_tab::Message::Card(
          char_id,
          character_card::Message::SkillTrainingPressed(id),
        ))
        .map(Message::CharactersTab);
      iced::Task::done(Message::CharactersTab(characters_tab::Message::NavigateToSkills(id)))
    }
    characters_tab::Message::Card(char_id, character_card::Message::TagsPressed(_)) => {
      handle_character_tags_pressed(state, char_id)
    }
    characters_tab::Message::Card(char_id, character_card::Message::WalletPressed(id)) => {
      let _ = state
        .character_pane
        .update(characters_tab::Message::Card(
          char_id,
          character_card::Message::WalletPressed(id),
        ))
        .map(Message::CharactersTab);
      iced::Task::done(Message::CharactersTab(characters_tab::Message::NavigateToWallet(id)))
    }
    characters_tab::Message::CharacterAdded(character) => handle_character_added(state, character, services),
    characters_tab::Message::CharacterPublicRefreshTick => character_public_refresh_task(state, services),
    characters_tab::Message::CharacterPublicRefreshed(updates) => handle_character_public_refreshed(state, updates),
    characters_tab::Message::CharacterTagsLoaded(id, tags) => handle_character_tags_loaded(state, id, tags),
    characters_tab::Message::ContextMenu(context_menu::Message::CopyName) => handle_copy_name(state),
    characters_tab::Message::LocationRefreshTick => location_refresh_task(state, services),
    characters_tab::Message::LocationsRefreshed(updates) => handle_locations_refreshed(state, updates),
    characters_tab::Message::RemoveCharacter(id) => {
      state.confirm_remove = Some(id);
      iced::Task::none()
    }
    msg => state.character_pane.update(msg).map(Message::CharactersTab),
  }
}

fn handle_character_tags_pressed(state: &mut State, char_id: i64) -> iced::Task<Message> {
  if let Some(c) = state.all_characters.iter().find(|c| *c.id() == char_id) {
    let name = c.name().clone();
    let existing = c.tags().clone();
    let modal = pod_ui::views::characters::tag_modal::State::new(char_id, "character", name, existing);
    let input_id = modal.input_id.clone();
    state.tag_modal = Some(modal);
    recompute_tag_corpus(state);
    let saved_offset = state.character_pane.scroll_offset();
    let focus = iced::widget::operation::focus(input_id).map(|_: ()| Message::TagsApplied);
    let restore = iced::widget::operation::scroll_to(characters_tab::GRID_SCROLL_ID.clone(), saved_offset);
    return focus.chain(restore);
  }
  iced::Task::none()
}

fn handle_character_added(state: &mut State, character: Character, services: &Services) -> iced::Task<Message> {
  if let Some(bytes) = character.portrait_data() {
    state
      .character_pane
      .portrait_handles
      .insert(*character.id(), image::Handle::from_bytes(bytes.clone()));
  }
  if let Some(existing) = state.all_characters.iter_mut().find(|c| c.id() == character.id()) {
    existing.set_access_token(character.access_token().clone());
    existing.set_refresh_token(character.refresh_token().clone());
    existing.set_token_expires_at(*character.token_expires_at());
    existing.set_granted_scopes(character.granted_scopes().clone());
    state.add_status = None;
    refilter(state);
    return iced::Task::none();
  }
  state.all_characters.push(character);
  state.add_status = None;
  refilter(state);
  character_public_refresh_task(state, services)
}

fn handle_character_public_refreshed(state: &mut State, updates: Vec<(i64, i64, String)>) -> iced::Task<Message> {
  for (id, corp_id, corp_name) in updates {
    if let Some(c) = state.all_characters.iter_mut().find(|c| *c.id() == id) {
      c.set_corp_id(corp_id);
      c.set_corp_name(corp_name);
    }
  }
  refilter(state);
  iced::Task::none()
}

fn handle_character_tags_loaded(state: &mut State, id: i64, tags: Vec<(i32, String)>) -> iced::Task<Message> {
  if let Some(c) = state.all_characters.iter_mut().find(|c| *c.id() == id) {
    *c.tags_mut() = tags;
  }
  refilter(state);
  recompute_tag_corpus(state);
  iced::Task::none()
}

fn handle_copy_name(state: &mut State) -> iced::Task<Message> {
  let name = state
    .character_pane
    .context_menu
    .as_ref()
    .map(|s| s.character_name.clone())
    .unwrap_or_default();
  let task = state
    .character_pane
    .update(characters_tab::Message::ContextMenu(context_menu::Message::CopyName))
    .map(Message::CharactersTab);
  iced::Task::batch([task, iced::clipboard::write(name)])
}

fn handle_locations_refreshed(
  state: &mut State,
  updates: Vec<(i64, Option<String>, Option<bool>)>,
) -> iced::Task<Message> {
  for (id, name, docked) in updates {
    if let Some(c) = state.all_characters.iter_mut().find(|c| *c.id() == id) {
      c.set_location_name(name);
      c.set_location_docked(docked);
    }
  }
  refilter(state);
  iced::Task::none()
}

fn update_characters_tab(state: &mut State, msg: characters_tab::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    characters_tab::Message::DragEnd => update_drag(state, services),
    characters_tab::Message::SkillQueueRefreshTick => skill_queue_refresh_task(state, services),
    characters_tab::Message::SkillQueuesRefreshed(updates) => update_skills_queue(state, updates),
    characters_tab::Message::WalletRefreshTick => wallet_refresh_task(state, services),
    characters_tab::Message::WalletsRefreshed(updates) => update_wallet(state, updates),
    msg => update_character_card(state, msg, services),
  }
}

fn update_confirm_remove(state: &mut State, services: &Services) -> iced::Task<Message> {
  let Some(character_id) = state.confirm_remove.take() else {
    return iced::Task::none();
  };
  state.all_characters.retain(|c| *c.id() != character_id);
  state.character_pane.portrait_handles.remove(&character_id);
  refilter(state);
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  iced::Task::perform(async move { db.characters().delete(character_id).await.ok() }, |_| {
    Message::TagsApplied
  })
}

fn update_confirm_remove_corporation(state: &mut State, services: &Services) -> iced::Task<Message> {
  let Some(corp_id) = state.confirm_remove_corporation.take() else {
    return iced::Task::none();
  };
  state.all_corporations.retain(|c| *c.id() != corp_id);
  state.corporations.retain(|c| *c.id() != corp_id);
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  iced::Task::perform(async move { db.corporations().delete(corp_id).await.ok() }, |_| {
    Message::TagsApplied
  })
}

fn update_corporation(state: &mut State, msg: corporations_tab::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    corporations_tab::Message::Card(corp_id, corporation_card::Message::TagsPressed(_)) => {
      handle_corp_tags_pressed(state, corp_id)
    }
    corporations_tab::Message::CorpPublicRefreshTick => corp_public_refresh_task(state, services),
    corporations_tab::Message::CorpPublicRefreshed(updated) => handle_corp_public_refreshed(state, updated),
    corporations_tab::Message::CorpWalletRefreshTick => corp_wallet_refresh_task(state, services),
    corporations_tab::Message::CorporationAdded(corp) => handle_corporation_added(state, corp),
    corporations_tab::Message::CorporationRemoved(id) => {
      state.all_corporations.retain(|c| *c.id() != id);
      state.corporations.retain(|c| *c.id() != id);
      iced::Task::none()
    }
    corporations_tab::Message::CorporationTagsLoaded(id, tags) => handle_corporation_tags_loaded(state, id, tags),
    corporations_tab::Message::CorporationsLoaded(corps) => handle_corporations_loaded(state, corps, services),
    corporations_tab::Message::HqNamesLoaded(resolved) => handle_hq_names_loaded(state, resolved),
    corporations_tab::Message::RemoveCorporation(corp_id) => handle_remove_corporation(state, corp_id, services),
    msg => state.corporation_pane.update(msg).map(Message::CorporationsTab),
  }
}

fn handle_corp_tags_pressed(state: &mut State, corp_id: i64) -> iced::Task<Message> {
  if let Some(c) = state.all_corporations.iter().find(|c| *c.id() == corp_id) {
    let name = c.name().clone();
    let existing = c.tags().clone();
    let modal = pod_ui::views::characters::tag_modal::State::new(corp_id, "corporation", name, existing);
    let input_id = modal.input_id.clone();
    state.tag_modal = Some(modal);
    recompute_tag_corpus(state);
    return iced::widget::operation::focus(input_id).map(|_: ()| Message::TagsApplied);
  }
  iced::Task::none()
}

fn handle_corp_public_refreshed(state: &mut State, updated: Vec<Corporation>) -> iced::Task<Message> {
  for corp in updated {
    if let Some(bytes) = corp.icon_data() {
      state
        .corporation_pane
        .icon_handles
        .insert(*corp.id(), iced::widget::image::Handle::from_bytes(bytes.clone()));
    }
    if let Some(existing) = state.all_corporations.iter_mut().find(|c| *c.id() == *corp.id()) {
      *existing = corp.clone();
    }
    if let Some(existing) = state.corporations.iter_mut().find(|c| *c.id() == *corp.id()) {
      *existing = corp;
    }
  }
  iced::Task::none()
}

fn handle_corporation_added(state: &mut State, corp: Corporation) -> iced::Task<Message> {
  if let Some(bytes) = corp.icon_data() {
    state
      .corporation_pane
      .icon_handles
      .insert(*corp.id(), iced::widget::image::Handle::from_bytes(bytes.clone()));
  }
  state.all_corporations.retain(|c| *c.id() != *corp.id());
  state.all_corporations.push(corp.clone());
  state.corporations.retain(|c| *c.id() != *corp.id());
  state.corporations.push(corp);
  state.add_status = None;
  iced::Task::none()
}

fn handle_corporation_tags_loaded(state: &mut State, id: i64, tags: Vec<(i32, String)>) -> iced::Task<Message> {
  if let Some(c) = state.all_corporations.iter_mut().find(|c| *c.id() == id) {
    *c.tags_mut() = tags.clone();
  }
  if let Some(c) = state.corporations.iter_mut().find(|c| *c.id() == id) {
    *c.tags_mut() = tags;
  }
  recompute_tag_corpus(state);
  iced::Task::none()
}

fn handle_corporations_loaded(state: &mut State, corps: Vec<Corporation>, services: &Services) -> iced::Task<Message> {
  state.corporation_pane.icon_handles = corps
    .iter()
    .filter_map(|c| {
      c.icon_data()
        .as_ref()
        .map(|b| (*c.id(), iced::widget::image::Handle::from_bytes(b.clone())))
    })
    .collect();
  state.all_corporations = corps.clone();
  state.corporations = corps.clone();
  let tag_tasks = corps_tag_tasks(&corps, services);
  let hq_task = corps_hq_task(&corps, services);
  iced::Task::batch([tag_tasks, hq_task])
}

/// Builds per-corporation tag loading tasks after corporations are loaded.
fn corps_tag_tasks(corps: &[Corporation], services: &Services) -> iced::Task<Message> {
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  let corp_ids: Vec<i64> = corps.iter().map(|c| *c.id()).collect();
  iced::Task::batch(corp_ids.into_iter().map(|corp_id| {
    let db = db.clone();
    iced::Task::perform(
      async move {
        let tags = db
          .tags()
          .tags_for_corporation(corp_id)
          .await
          .unwrap_or_default()
          .into_iter()
          .map(|t| (t.id, t.name))
          .collect();
        (corp_id, tags)
      },
      |(id, tags)| Message::CorporationsTab(corporations_tab::Message::CorporationTagsLoaded(id, tags)),
    )
  }))
}

/// Builds the HQ station name resolution task after corporations are loaded.
fn corps_hq_task(corps: &[Corporation], services: &Services) -> iced::Task<Message> {
  let Some(esi) = services.esi_client.clone() else {
    return iced::Task::none();
  };
  let corps_with_hq: Vec<(i64, i64)> = corps
    .iter()
    .filter_map(|c| (*c.home_station_id()).map(|sid| (*c.id(), sid)))
    .collect();
  if corps_with_hq.is_empty() {
    return iced::Task::none();
  }
  iced::Task::perform(
    async move {
      let mut results = Vec::new();
      for (corp_id, station_id) in corps_with_hq {
        if let Ok(station) = esi.universe().station(station_id).await {
          results.push((corp_id, station.name));
        }
      }
      results
    },
    |resolved| Message::CorporationsTab(corporations_tab::Message::HqNamesLoaded(resolved)),
  )
}

fn handle_hq_names_loaded(state: &mut State, resolved: Vec<(i64, String)>) -> iced::Task<Message> {
  for (corp_id, name) in resolved {
    if let Some(c) = state.all_corporations.iter_mut().find(|c| *c.id() == corp_id) {
      *c.hq_name_mut() = Some(name.clone());
    }
    if let Some(c) = state.corporations.iter_mut().find(|c| *c.id() == corp_id) {
      *c.hq_name_mut() = Some(name);
    }
  }
  iced::Task::none()
}

fn handle_remove_corporation(state: &mut State, corp_id: i64, services: &Services) -> iced::Task<Message> {
  state.corporations.retain(|c| *c.id() != corp_id);
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  iced::Task::perform(
    async move {
      db.corporations().delete(corp_id).await.ok();
      corp_id
    },
    |id| Message::CorporationsTab(corporations_tab::Message::CorporationRemoved(id)),
  )
}

fn update_drag(state: &mut State, services: &Services) -> iced::Task<Message> {
  let dragging = state.character_pane.dragging_id;
  let hover = state.character_pane.drag_hover;
  let pane_task = state
    .character_pane
    .update(characters_tab::Message::DragEnd)
    .map(Message::CharactersTab);
  if let (Some(dragging_id), Some(hover_id)) = (dragging, hover)
    && dragging_id != hover_id
  {
    let ids: Vec<i64> = state.all_characters.iter().map(|c| *c.id()).collect();
    let new_order = reorder_ids(&ids, dragging_id, hover_id);
    reorder_characters_by_ids(&mut state.all_characters, &new_order);
    refilter(state);
    if let Some(db) = services.db.clone() {
      let updates: Vec<(i64, i32)> = new_order.iter().enumerate().map(|(i, &id)| (id, i as i32)).collect();
      let db_task = iced::Task::perform(
        async move { db.characters().update_sort_orders(&updates).await.ok() },
        |_| Message::TagsApplied,
      );
      return iced::Task::batch([pane_task, db_task]);
    }
  }
  pane_task
}

fn update_header(state: &mut State, msg: header::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    header::Message::AddCharacter => {
      let Some(esi) = services.esi_client.clone() else {
        state.add_status = Some("ESI client not available".to_string());
        return iced::Task::none();
      };
      let (url, verifier, oauth_state) = esi
        .auth()
        .sign_in(pod_esi::scopes::Scopes::ALL, "http://127.0.0.1:47823/callback");
      let _ = open::that_detached(&url);
      state.add_status = Some("Waiting for browser login\u{2026}".to_string());
      let db = services.db.clone();
      iced::Task::perform(
        async move { add_character(esi, verifier, oauth_state, db).await },
        |result| match result {
          Ok(character) => Message::CharactersTab(characters_tab::Message::CharacterAdded(character)),
          Err(e) => Message::AddCharacterError(e),
        },
      )
    }
    header::Message::AddCorporation => {
      let Some(esi) = services.esi_client.clone() else {
        state.add_status = Some("ESI client not available".to_string());
        return iced::Task::none();
      };
      let corp_scopes = services.config.features().required_scopes_for_corporation();
      let (url, verifier, oauth_state) = esi.auth().sign_in(&corp_scopes, "http://127.0.0.1:47823/callback");
      let _ = open::that_detached(&url);
      state.add_status = Some("Waiting for browser login\u{2026}".to_string());
      let db = services.db.clone();
      iced::Task::perform(
        async move { add_corporation(esi, verifier, oauth_state, db).await },
        |result| match result {
          Ok(corp) => Message::CorporationsTab(corporations_tab::Message::CorporationAdded(corp)),
          Err(e) => Message::AddCorporationError(e),
        },
      )
    }
    header::Message::TabSelected(id) => {
      state.active_tab = match id.as_str() {
        "corporations" => pod_ui::views::characters::Tab::Corporations,
        _ => pod_ui::views::characters::Tab::Characters,
      };
      state.search_filter = search_filter::State::new();
      iced::Task::none()
    }
  }
}

fn update_search_filter(state: &mut State, msg: search_filter::Message) -> iced::Task<Message> {
  if let search_filter::Message::QueryChanged(ref q) = msg {
    let q = q.clone();
    let task = state.search_filter.update(msg).map(Message::SearchFilter);
    match state.active_tab {
      pod_ui::views::characters::Tab::Characters => {
        state.characters = filter_characters(&state.all_characters, &q);
      }
      pod_ui::views::characters::Tab::Corporations => {
        state.corporations = filter_corporations(&state.all_corporations, &q);
      }
    }
    return task;
  }
  if let search_filter::Message::FocusInput = msg {
    return iced::widget::operation::focus(state.search_filter.input_id.clone());
  }
  state.search_filter.update(msg).map(Message::SearchFilter)
}

fn update_skills_queue(
  state: &mut State,
  updates: Vec<(i64, Vec<CharacterSkill>, Vec<TrainingQueueEntry>)>,
) -> iced::Task<Message> {
  for (id, skills, tq) in updates {
    let Some(c) = state.all_characters.iter_mut().find(|c| *c.id() == id) else {
      continue;
    };
    let skill_map: HashMap<i32, &CharacterSkill> = skills.iter().map(|s| (s.skill_id, s)).collect();
    for s in c.skills_mut().iter_mut() {
      s.is_active_training = false;
      if let Some(updated) = skill_map.get(&s.skill_id) {
        s.active_level = updated.active_level;
        s.is_active_training = updated.is_active_training;
        s.trained_level = updated.trained_level;
        s.training_end_time = updated.training_end_time;
        s.training_level_end_sp = updated.training_level_end_sp;
        s.training_level_start_sp = updated.training_level_start_sp;
        s.training_start_sp = updated.training_start_sp;
        s.training_start_time = updated.training_start_time;
      }
    }
    *c.training_queue_mut() = tq;
  }
  refilter(state);
  iced::Task::none()
}

fn update_wallet(state: &mut State, updates: Vec<(i64, Option<f64>)>) -> iced::Task<Message> {
  for (id, balance) in updates {
    if let Some(c) = state.all_characters.iter_mut().find(|c| *c.id() == id) {
      c.set_isk_balance(balance);
    }
  }
  refilter(state);
  iced::Task::none()
}

#[cfg(test)]
mod tests {
  use pod_model::{Character, Corporation};

  use super::*;

  fn make_character(id: i64, name: &str) -> Character {
    Character::new(id, name)
  }

  fn make_corporation(id: i64, name: &str, ticker: &str) -> Corporation {
    let mut corp = Corporation::new(id, name);
    corp.set_ticker(ticker);
    corp
  }

  fn make_services() -> Services {
    Services {
      config: crate::config::Settings::default(),
      db: None,
      esi_client: None,
    }
  }

  fn make_state(characters: Vec<Character>) -> State {
    State {
      active_tab: Default::default(),
      add_status: None,
      all_characters: characters.clone(),
      all_corporations: Vec::new(),
      all_tags: Vec::new(),
      character_pane: characters_tab::State::new(),
      characters,
      confirm_remove: None,
      confirm_remove_corporation: None,
      corporation_pane: Default::default(),
      corporations: Vec::new(),
      feat_location_tracking: true,
      feat_skill_monitoring: true,
      feat_wallet: true,
      header: Default::default(),
      search_filter: search_filter::State::new(),
      tag_corpus: Vec::new(),
      tag_modal: None,
    }
  }

  mod reorder_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_moves_element_forward() {
      let ids = vec![1i64, 2, 3, 4];
      let result = reorder_ids(&ids, 1, 3);

      assert_eq!(result, vec![2i64, 3, 1, 4]);
    }

    #[test]
    fn it_moves_element_backward() {
      let ids = vec![1i64, 2, 3, 4];
      let result = reorder_ids(&ids, 4, 2);

      assert_eq!(result, vec![1i64, 4, 2, 3]);
    }

    #[test]
    fn it_is_no_op_when_dragging_to_same_position() {
      let ids = vec![1i64, 2, 3];
      let result = reorder_ids(&ids, 2, 2);

      assert_eq!(result, vec![1i64, 2, 3]);
    }

    #[test]
    fn it_handles_move_to_end() {
      let ids = vec![1i64, 2, 3, 4];
      let result = reorder_ids(&ids, 1, 4);

      assert_eq!(result, vec![2i64, 3, 4, 1]);
    }

    #[test]
    fn it_handles_move_to_start() {
      let ids = vec![1i64, 2, 3, 4];
      let result = reorder_ids(&ids, 4, 1);

      assert_eq!(result, vec![4i64, 1, 2, 3]);
    }
  }

  mod reorder_characters_by_ids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reorders_by_given_id_sequence() {
      let mut chars = vec![
        make_character(1, "Alpha"),
        make_character(2, "Beta"),
        make_character(3, "Gamma"),
      ];
      let order = vec![3i64, 1, 2];
      reorder_characters_by_ids(&mut chars, &order);

      assert_eq!(*chars[0].id(), 3);
      assert_eq!(*chars[1].id(), 1);
      assert_eq!(*chars[2].id(), 2);
    }

    #[test]
    fn it_skips_ids_not_in_the_list() {
      let mut chars = vec![make_character(1, "Alpha"), make_character(2, "Beta")];
      let order = vec![1i64];
      reorder_characters_by_ids(&mut chars, &order);

      assert_eq!(chars.len(), 1);
      assert_eq!(*chars[0].id(), 1);
    }
  }

  mod filter_corporations {
    use pretty_assertions::assert_eq;

    use super::*;

    fn corp_with_alliance(id: i64, name: &str, ticker: &str, alliance: &str) -> Corporation {
      let mut corp = make_corporation(id, name, ticker);
      corp.set_alliance_name(Some(alliance.to_string()));
      corp
    }

    #[test]
    fn it_returns_all_when_query_is_empty() {
      let corps = vec![
        make_corporation(1, "Goonswarm Federation", "CONDI"),
        make_corporation(2, "TEST Alliance Please Ignore", "TEST"),
      ];
      let result = filter_corporations(&corps, "");

      assert_eq!(result.len(), 2);
    }

    #[test]
    fn it_returns_all_when_query_is_whitespace() {
      let corps = vec![make_corporation(1, "Goonswarm Federation", "CONDI")];
      let result = filter_corporations(&corps, "   ");

      assert_eq!(result.len(), 1);
    }

    #[test]
    fn it_filters_by_name_case_insensitive() {
      let corps = vec![
        make_corporation(1, "Goonswarm Federation", "CONDI"),
        make_corporation(2, "TEST Alliance Please Ignore", "TEST"),
      ];
      let result = filter_corporations(&corps, "goon");

      assert_eq!(result.len(), 1);
      assert_eq!(*result[0].id(), 1);
    }

    #[test]
    fn it_filters_by_ticker() {
      let corps = vec![
        make_corporation(1, "Goonswarm Federation", "CONDI"),
        make_corporation(2, "TEST Alliance Please Ignore", "TEST"),
      ];
      let result = filter_corporations(&corps, "condi");

      assert_eq!(result.len(), 1);
      assert_eq!(*result[0].id(), 1);
    }

    #[test]
    fn it_filters_by_alliance_name() {
      let corps = vec![
        corp_with_alliance(1, "Some Corp", "SOME", "The Imperium"),
        make_corporation(2, "Other Corp", "OTHR"),
      ];
      let result = filter_corporations(&corps, "imperium");

      assert_eq!(result.len(), 1);
      assert_eq!(*result[0].id(), 1);
    }

    #[test]
    fn it_returns_empty_when_no_match() {
      let corps = vec![make_corporation(1, "Goonswarm", "CONDI")];
      let result = filter_corporations(&corps, "zzz_no_match");

      assert!(result.is_empty());
    }
  }

  mod build_tag_corpus {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_empty_for_no_characters() {
      let corpus = build_tag_corpus(&[]);

      assert!(corpus.is_empty());
    }

    #[test]
    fn it_counts_tags_across_characters() {
      let mut c1 = make_character(1, "Alpha");
      *c1.tags_mut() = vec![(1, "pvp".to_string()), (2, "highsec".to_string())];
      let mut c2 = make_character(2, "Beta");
      *c2.tags_mut() = vec![(1, "pvp".to_string())];

      let corpus = build_tag_corpus(&[c1, c2]);

      let pvp = corpus.iter().find(|(name, _)| name == "pvp");
      assert_eq!(pvp, Some(&("pvp".to_string(), 2)));
      let highsec = corpus.iter().find(|(name, _)| name == "highsec");
      assert_eq!(highsec, Some(&("highsec".to_string(), 1)));
    }

    #[test]
    fn it_sorts_by_count_descending_then_name_ascending() {
      let mut c1 = make_character(1, "Alpha");
      *c1.tags_mut() = vec![(1, "rare".to_string())];
      let mut c2 = make_character(2, "Beta");
      *c2.tags_mut() = vec![(2, "common".to_string())];
      let mut c3 = make_character(3, "Gamma");
      *c3.tags_mut() = vec![(2, "common".to_string())];

      let corpus = build_tag_corpus(&[c1, c2, c3]);

      assert_eq!(corpus[0].0, "common");
      assert_eq!(corpus[0].1, 2);
      assert_eq!(corpus[1].0, "rare");
      assert_eq!(corpus[1].1, 1);
    }
  }

  mod build_corp_tag_corpus {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_empty_for_no_corporations() {
      let corpus = build_corp_tag_corpus(&[]);

      assert!(corpus.is_empty());
    }

    #[test]
    fn it_counts_tags_across_corporations() {
      let mut c1 = make_corporation(1, "Corp A", "CA");
      *c1.tags_mut() = vec![(1, "industrial".to_string()), (2, "sov".to_string())];
      let mut c2 = make_corporation(2, "Corp B", "CB");
      *c2.tags_mut() = vec![(1, "industrial".to_string())];

      let corpus = build_corp_tag_corpus(&[c1, c2]);

      let industrial = corpus.iter().find(|(name, _)| name == "industrial");
      assert_eq!(industrial, Some(&("industrial".to_string(), 2)));
      let sov = corpus.iter().find(|(name, _)| name == "sov");
      assert_eq!(sov, Some(&("sov".to_string(), 1)));
    }

    #[test]
    fn it_sorts_by_count_descending_then_name_ascending() {
      let mut c1 = make_corporation(1, "Corp A", "CA");
      *c1.tags_mut() = vec![(1, "mining".to_string())];
      let mut c2 = make_corporation(2, "Corp B", "CB");
      *c2.tags_mut() = vec![(2, "pvp".to_string())];
      let mut c3 = make_corporation(3, "Corp C", "CC");
      *c3.tags_mut() = vec![(2, "pvp".to_string())];

      let corpus = build_corp_tag_corpus(&[c1, c2, c3]);

      assert_eq!(corpus[0].0, "pvp");
      assert_eq!(corpus[1].0, "mining");
    }
  }

  mod update_character_card {
    use super::*;

    mod character_added {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_adds_a_new_character() {
        let mut state = make_state(vec![make_character(1, "Alpha")]);
        let services = make_services();

        let _ = update_character_card(
          &mut state,
          characters_tab::Message::CharacterAdded(make_character(2, "Beta")),
          &services,
        );

        assert_eq!(state.all_characters.len(), 2);
      }

      #[test]
      fn it_does_not_add_a_duplicate_when_character_already_exists() {
        let mut state = make_state(vec![make_character(1, "Alpha")]);
        let services = make_services();

        let _ = update_character_card(
          &mut state,
          characters_tab::Message::CharacterAdded(make_character(1, "Alpha")),
          &services,
        );

        assert_eq!(state.all_characters.len(), 1);
      }

      #[test]
      fn it_updates_token_on_reauth_for_existing_character() {
        let mut existing = make_character(1, "Alpha");
        existing.set_access_token("old-token");
        let mut state = make_state(vec![existing]);
        let services = make_services();

        let mut reauthed = make_character(1, "Alpha");
        reauthed.set_access_token("new-token");
        reauthed.set_refresh_token("new-refresh");
        reauthed.set_granted_scopes(Some("esi-skills.read_skills.v1".to_string()));

        let _ = update_character_card(&mut state, characters_tab::Message::CharacterAdded(reauthed), &services);

        assert_eq!(state.all_characters.len(), 1);
        assert_eq!(state.all_characters[0].access_token(), "new-token");
        assert_eq!(state.all_characters[0].refresh_token(), "new-refresh");
        assert_eq!(
          state.all_characters[0].granted_scopes().as_deref(),
          Some("esi-skills.read_skills.v1")
        );
      }

      #[test]
      fn it_clears_add_status_on_reauth() {
        let mut state = make_state(vec![make_character(1, "Alpha")]);
        state.add_status = Some("Waiting for browser login\u{2026}".to_string());
        let services = make_services();

        let _ = update_character_card(
          &mut state,
          characters_tab::Message::CharacterAdded(make_character(1, "Alpha")),
          &services,
        );

        assert_eq!(state.add_status, None);
      }
    }
  }
}
