//! Main window controller: navigation shell that routes to child view controllers.

use iced::Subscription;
use pod_model::{Character, CharacterSkill, Corporation};
pub use pod_ui::views::main_window::{ActiveView, Message, Nav, State};
use pod_ui::{
  components::{character_picker, status_bar},
  views::{assets, character_detail, characters, mail, settings, skills, wallet},
};

use crate::{
  controllers::{
    assets as assets_ctrl, character_detail as character_detail_ctrl, characters as characters_ctrl, mail as mail_ctrl,
    settings as settings_ctrl, skills as skills_ctrl, wallet as wallet_ctrl,
  },
  services::Services,
};

/// Creates a new shell state and a startup task that initializes the default view.
pub fn new(
  characters: Vec<Character>,
  services: &Services,
  skills_left_pane_width: Option<f32>,
  mail_folder_pane_width: Option<f32>,
  mail_message_list_width: Option<f32>,
  wallet_right_rail_width: Option<f32>,
) -> (State, iced::Task<Message>) {
  let features = services.config.features();
  let (chars_state, chars_task) = characters_ctrl::new(characters.clone(), services);
  let state = State {
    active_nav: Nav::Characters,
    active_view: ActiveView::Characters(chars_state),
    characters,
    corporations: Vec::new(),
    esi_connected: true,
    eve_time: utc_time_string(),
    feat_asset_tracking: *features.asset_tracking(),
    feat_mail: *features.mail(),
    feat_skill_monitoring: *features.skill_monitoring(),
    feat_wallet: *features.wallet(),
    hovered_nav: None,
    mail_folder_pane_width: mail_folder_pane_width.unwrap_or(240.0),
    mail_message_list_width: mail_message_list_width.unwrap_or(380.0),
    refresh_successes: 0,
    skills_left_pane_width: skills_left_pane_width.unwrap_or(700.0),
    sync: status_bar::SyncState::default(),
    toast: None,
    wallet_right_rail_width: wallet_right_rail_width.unwrap_or(220.0),
  };
  (state, chars_task.map(Message::Characters))
}

/// Returns background subscriptions for the currently active view.
pub fn subscription(state: &State) -> Subscription<Message> {
  match &state.active_view {
    ActiveView::Assets(s) => assets::subscription(s).map(Message::Assets),
    ActiveView::Characters(chars_state) => characters_ctrl::subscription(chars_state).map(Message::Characters),
    ActiveView::Mail(s) => mail::subscription(s).map(Message::Mail),
    ActiveView::Settings(_) => Subscription::none(),
    ActiveView::Skills(s) => skills_ctrl::subscription(s).map(Message::Skills),
    ActiveView::Wallet(s) => wallet::subscription(s).map(Message::Wallet),
    _ => Subscription::none(),
  }
}

/// Processes a main window message and returns a task.
///
/// Returns `(task, Option<new_config>)` — the config is `Some` whenever a
/// settings toggle or reset just ran, so the caller can update `app.config`.
pub fn update(
  state: &mut State,
  message: Message,
  services: &Services,
) -> (iced::Task<Message>, Option<crate::config::Settings>) {
  match message {
    Message::Assets(msg) => (update_assets(state, msg, services), None),
    Message::CharacterDetail(msg) => (update_character_detail(state, msg, services), None),
    Message::Characters(msg) => (update_characters(state, msg, services), None),
    Message::DismissToast => {
      state.toast = None;
      (iced::Task::none(), None)
    }
    Message::EveTimeTick => {
      state.eve_time = utc_time_string();
      (iced::Task::none(), None)
    }
    Message::HoverNav(nav) => {
      state.hovered_nav = nav;
      (iced::Task::none(), None)
    }
    Message::Mail(msg) => (update_mail(state, msg, services), None),
    Message::Navigate(nav) => (update_navigate(state, nav, services), None),
    Message::RefreshAll => (update_refresh_all(state, services), None),
    Message::Settings(msg) => update_settings(state, msg, services),
    Message::ShowToast(msg) => {
      state.toast = Some(msg);
      let task = iced::Task::perform(
        async { tokio::time::sleep(std::time::Duration::from_millis(2500)).await },
        |()| Message::DismissToast,
      );
      (task, None)
    }
    Message::Skills(msg) => (update_skills(state, msg, services), None),
    Message::StatusBar(status_bar::Message::RefreshPressed) => {
      let (task, cfg) = update(state, Message::RefreshAll, services);
      (task, cfg)
    }
    Message::Wallet(msg) => (update_wallet(state, msg, services), None),
  }
}

fn update_assets(state: &mut State, msg: assets::Message, services: &Services) -> iced::Task<Message> {
  if let assets::Message::ReauthorizeCharacter(_) = &msg {
    return trigger_reauth(services);
  }
  let ActiveView::Assets(s) = &mut state.active_view else {
    return iced::Task::none();
  };
  if let assets::Message::FetchCorpAssets(corp_id) = &msg {
    let corp_id = *corp_id;
    return assets_ctrl::fetch_corp_assets(corp_id, s, services).map(Message::Assets);
  }
  if let assets::Message::RefreshNavHistory = &msg {
    if let (Some(db), Some(esi)) = (services.db.clone(), services.esi_client.clone()) {
      let char_ids: Vec<i64> = s.characters.iter().map(|c| *c.id()).collect();
      return iced::Task::perform(
        async move {
          crate::services::prices::sync(&db, &esi).await;
          assets_ctrl::nav_history(db, char_ids, 90).await
        },
        |data| Message::Assets(assets::Message::NavHistoryLoaded(data)),
      );
    }
    return iced::Task::none();
  }
  let should_refresh_values = matches!(&msg, assets::Message::TabSelected(assets::Tab::Values))
    || (matches!(
      &msg,
      assets::Message::AssetsLoaded(_) | assets::Message::CorpAssetsLoaded(_)
    ) && s.active_tab == assets::Tab::Values);
  if let assets::Message::StockpilesTab(assets::stockpiles_tab::Message::FormSave) = &msg
    && let (Some(form), Some(db)) = (s.stockpile_form.clone(), services.db.clone())
  {
    let name = form.name.clone();
    let location_id = form.location_id_text.trim().parse::<i64>().ok();
    let items: Vec<(i32, i32)> = form
      .items
      .iter()
      .filter_map(|it| {
        let t = it.type_id_text.trim().parse::<i32>().ok()?;
        let q = it.qty_text.trim().parse::<i32>().ok()?;
        Some((t, q))
      })
      .collect();
    let base_task = assets::update(s, msg).map(Message::Assets);
    if let Some(editing_id) = form.editing_id {
      let task = iced::Task::perform(
        assets_ctrl::update_stockpile(db, editing_id, name, location_id, None, items),
        |piles| Message::Assets(assets::Message::StockpilesLoaded(piles)),
      );
      s.stockpile_form = None;
      return iced::Task::batch([base_task, task]);
    } else {
      let task = iced::Task::perform(
        assets_ctrl::create_stockpile(db, name, location_id, None, items),
        |piles| Message::Assets(assets::Message::StockpilesLoaded(piles)),
      );
      s.stockpile_form = None;
      return iced::Task::batch([base_task, task]);
    }
  }
  if let assets::Message::StockpilesTab(assets::stockpiles_tab::Message::DeleteStockpile(id)) = &msg {
    let id = *id;
    if let Some(db) = services.db.clone() {
      let base_task = assets::update(s, msg).map(Message::Assets);
      let task = iced::Task::perform(assets_ctrl::delete_stockpile(db, id), |piles| {
        Message::Assets(assets::Message::StockpilesLoaded(piles))
      });
      return iced::Task::batch([base_task, task]);
    }
  }
  let new_items: Option<Vec<(i32, String)>> = match &msg {
    assets::Message::AssetsLoaded(records) | assets::Message::CorpAssetsLoaded(records) => {
      let items: Vec<(i32, String)> = records
        .iter()
        .map(|r| (r.type_id, r.icon_variant.clone()))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .filter(|(type_id, variant)| !s.item_icons.contains_key(&(*type_id, variant.clone())))
        .collect();
      if items.is_empty() { None } else { Some(items) }
    }
    _ => None,
  };
  let base_task = assets::update(s, msg).map(Message::Assets);
  let values_task = if should_refresh_values {
    if let Some(db) = services.db.clone() {
      let assets_snapshot = s.assets.clone();
      let chars_snapshot = state.characters.clone();
      s.values_loading = true;
      Some(iced::Task::perform(
        assets_ctrl::asset_values_breakdown(assets_snapshot, chars_snapshot, db),
        |data| Message::Assets(assets::Message::ValuesLoaded(data)),
      ))
    } else {
      None
    }
  } else {
    None
  };
  let icon_task =
    if let (Some(items), Some(esi), Some(db)) = (new_items, services.esi_client.clone(), services.db.clone()) {
      Some(iced::Task::perform(
        async move { assets_ctrl::fetch_type_icons(items, esi, db).await },
        |icons| Message::Assets(assets::Message::ItemIconsLoaded(icons)),
      ))
    } else {
      None
    };
  let mut tasks = vec![base_task];
  if let Some(vt) = values_task {
    tasks.push(vt);
  }
  if let Some(it) = icon_task {
    tasks.push(it);
  }
  iced::Task::batch(tasks)
}

fn update_character_detail(
  state: &mut State,
  msg: character_detail::Message,
  services: &Services,
) -> iced::Task<Message> {
  match &msg {
    character_detail::Message::CharacterSwitched(char_id) => {
      let char_id = *char_id;
      let Some(character) = state.characters.iter().find(|c| *c.id() == char_id).cloned() else {
        return iced::Task::none();
      };
      let (detail_state, detail_task) =
        character_detail_ctrl::new(char_id, character, state.characters.clone(), services);
      state.active_view = ActiveView::CharacterDetail(detail_state);
      return detail_task.map(Message::CharacterDetail);
    }
    character_detail::Message::Picker(character_picker::Message::Select(
      character_picker::PickerSelection::Character(char_id),
    )) => {
      let char_id = *char_id;
      let Some(character) = state.characters.iter().find(|c| *c.id() == char_id).cloned() else {
        return iced::Task::none();
      };
      let (detail_state, detail_task) =
        character_detail_ctrl::new(char_id, character, state.characters.clone(), services);
      state.active_view = ActiveView::CharacterDetail(detail_state);
      return detail_task.map(Message::CharacterDetail);
    }
    character_detail::Message::ReauthorizeCharacter(_char_id) => {
      return trigger_reauth(services);
    }
    _ => {}
  }
  let ActiveView::CharacterDetail(s) = &mut state.active_view else {
    return iced::Task::none();
  };
  character_detail_ctrl::update(s, msg, services).map(Message::CharacterDetail)
}

fn update_characters(state: &mut State, msg: characters::Message, services: &Services) -> iced::Task<Message> {
  let is_drag_end = matches!(
    &msg,
    characters::Message::CharactersTab(m)
      if matches!(m, characters::characters_tab::Message::DragEnd)
  );
  match &msg {
    characters::Message::CharactersTab(characters::characters_tab::Message::NavigateToDetail(char_id)) => {
      let char_id = *char_id;
      let Some(character) = state.characters.iter().find(|c| *c.id() == char_id).cloned() else {
        return iced::Task::none();
      };
      state.active_nav = Nav::Characters;
      let (detail_state, detail_task) =
        character_detail_ctrl::new(char_id, character, state.characters.clone(), services);
      state.active_view = ActiveView::CharacterDetail(detail_state);
      return detail_task.map(Message::CharacterDetail);
    }
    characters::Message::CharactersTab(characters::characters_tab::Message::NavigateToSkills(char_id)) => {
      let char_id = *char_id;
      state.active_nav = Nav::Skills;
      let (mut s, task) = skills_ctrl::new(state.characters.clone(), state.skills_left_pane_width, services);
      let _ = skills_ctrl::update(
        &mut s,
        skills::Message::Picker(character_picker::Message::Select(
          character_picker::PickerSelection::Character(char_id),
        )),
        services,
      );
      state.active_view = ActiveView::Skills(s);
      return task.map(Message::Skills);
    }
    characters::Message::CharactersTab(characters::characters_tab::Message::NavigateToWallet(char_id)) => {
      let char_id = *char_id;
      state.active_nav = Nav::Wallet;
      let (mut s, task) = wallet_ctrl::new(
        state.characters.clone(),
        state.corporations.clone(),
        services,
        state.wallet_right_rail_width,
      );
      let _ = wallet::update(
        &mut s,
        wallet::Message::CharacterPicker(character_picker::Message::Select(
          character_picker::PickerSelection::Character(char_id),
        )),
      );
      state.active_view = ActiveView::Wallet(s);
      return task.map(Message::Wallet);
    }
    characters::Message::CharactersTab(characters::characters_tab::Message::CharacterAdded(c))
      if !state.characters.iter().any(|existing| existing.id() == c.id()) =>
    {
      state.characters.push(c.clone());
    }
    characters::Message::ConfirmRemove => {
      if let ActiveView::Characters(s) = &state.active_view
        && let Some(id) = s.confirm_remove
      {
        state.characters.retain(|c| *c.id() != id);
      }
    }
    characters::Message::CharactersTab(characters::characters_tab::Message::LocationsRefreshed(updates)) => {
      update_sync_state(state, !updates.is_empty());
    }
    characters::Message::CharactersTab(characters::characters_tab::Message::SkillQueuesRefreshed(updates)) => {
      update_sync_state(state, !updates.is_empty());
      for (id, skills, tq) in updates {
        let Some(c) = state.characters.iter_mut().find(|c| *c.id() == *id) else {
          continue;
        };
        let skill_map: std::collections::HashMap<i32, &CharacterSkill> =
          skills.iter().map(|s| (s.skill_id, s)).collect();
        for s in c.skills_mut().iter_mut() {
          s.is_active_training = false;
          if let Some(updated) = skill_map.get(&s.skill_id) {
            s.is_active_training = updated.is_active_training;
            s.skill_name = updated.skill_name.clone();
            s.skillpoints = updated.skillpoints;
            s.trained_level = updated.trained_level;
            s.training_end_time = updated.training_end_time;
            s.training_level_end_sp = updated.training_level_end_sp;
            s.training_level_start_sp = updated.training_level_start_sp;
            s.training_start_sp = updated.training_start_sp;
            s.training_start_time = updated.training_start_time;
          }
        }
        *c.training_queue_mut() = tq.clone();
      }
      if let ActiveView::Skills(s) = &mut state.active_view {
        let updated_chars = state.characters.clone();
        skills_ctrl::refresh_characters(s, updated_chars);
      }
    }
    characters::Message::ConfirmRemoveCorporation => {
      if let ActiveView::Characters(s) = &state.active_view
        && let Some(id) = s.confirm_remove_corporation
      {
        state.corporations.retain(|c: &Corporation| *c.id() != id);
      }
    }
    characters::Message::CorporationsTab(characters::corporations_tab::Message::CorporationAdded(corp)) => {
      state.corporations.retain(|c: &Corporation| *c.id() != *corp.id());
      state.corporations.push(corp.clone());
    }
    characters::Message::CorporationsTab(characters::corporations_tab::Message::CorporationRemoved(id)) => {
      state.corporations.retain(|c: &Corporation| *c.id() != *id);
    }
    characters::Message::CorporationsTab(characters::corporations_tab::Message::CorporationsLoaded(corps)) => {
      state.corporations = corps.clone();
    }
    characters::Message::CharactersTab(characters::characters_tab::Message::WalletsRefreshed(updates)) => {
      update_sync_state(state, !updates.is_empty());
    }
    _ => {}
  }
  let ActiveView::Characters(chars_state) = &mut state.active_view else {
    return iced::Task::none();
  };
  let task = characters_ctrl::update(chars_state, msg, services).map(Message::Characters);
  if is_drag_end {
    let new_order: Vec<i64> = chars_state.all_characters.iter().map(|c| *c.id()).collect();
    state
      .characters
      .sort_by_key(|c| new_order.iter().position(|&id| id == *c.id()).unwrap_or(usize::MAX));
  }
  task
}

fn update_mail(state: &mut State, msg: mail::Message, services: &Services) -> iced::Task<Message> {
  if let mail::Message::ReauthorizeCharacter(_) = &msg {
    return trigger_reauth(services);
  }
  let is_drag_end = matches!(&msg, mail::Message::PaneDragEnd);
  let ActiveView::Mail(s) = &mut state.active_view else {
    return iced::Task::none();
  };
  let task = mail_ctrl::update(s, msg, services).map(Message::Mail);
  if is_drag_end {
    state.mail_folder_pane_width = s.folder_pane_width;
    state.mail_message_list_width = s.message_list_width;
  }
  task
}

fn update_navigate(state: &mut State, nav: Nav, services: &Services) -> iced::Task<Message> {
  state.active_nav = nav;
  match nav {
    Nav::Assets => {
      let (s, task) = assets_ctrl::new(state.characters.clone(), state.corporations.clone(), services);
      state.active_view = ActiveView::Assets(s);
      task.map(Message::Assets)
    }
    Nav::Characters => {
      let (chars_state, init_task) = characters_ctrl::new(state.characters.clone(), services);
      state.active_view = ActiveView::Characters(chars_state);
      init_task.map(Message::Characters)
    }
    Nav::Mail => {
      let (s, task) = mail_ctrl::new(
        state.characters.clone(),
        services,
        state.mail_folder_pane_width,
        state.mail_message_list_width,
      );
      state.active_view = ActiveView::Mail(s);
      task.map(Message::Mail)
    }
    Nav::Settings => {
      let (s, task) = settings_ctrl::new(services.config.features());
      state.active_view = ActiveView::Settings(s);
      task.map(Message::Settings)
    }
    Nav::Skills => {
      let (s, task) = skills_ctrl::new(state.characters.clone(), state.skills_left_pane_width, services);
      state.active_view = ActiveView::Skills(s);
      task.map(Message::Skills)
    }
    Nav::Wallet => {
      let (s, task) = wallet_ctrl::new(
        state.characters.clone(),
        state.corporations.clone(),
        services,
        state.wallet_right_rail_width,
      );
      state.active_view = ActiveView::Wallet(s);
      task.map(Message::Wallet)
    }
  }
}

fn update_refresh_all(state: &mut State, services: &Services) -> iced::Task<Message> {
  state.sync.start(3);
  state.refresh_successes = 0;
  let ActiveView::Characters(chars_state) = &state.active_view else {
    return iced::Task::none();
  };
  iced::Task::batch([
    characters_ctrl::location_refresh_task(chars_state, services).map(Message::Characters),
    characters_ctrl::skill_queue_refresh_task(chars_state, services).map(Message::Characters),
    characters_ctrl::wallet_refresh_task(chars_state, services).map(Message::Characters),
  ])
}

fn update_settings(
  state: &mut State,
  msg: settings::Message,
  services: &Services,
) -> (iced::Task<Message>, Option<crate::config::Settings>) {
  let is_save = matches!(
    &msg,
    settings::Message::ToggleFeature(_) | settings::Message::ResetDefaults
  );

  let (settings_task, updated_cfg) = {
    let ActiveView::Settings(s) = &mut state.active_view else {
      return (iced::Task::none(), None);
    };
    let task = settings_ctrl::update(s, msg, services).map(Message::Settings);
    let cfg = if is_save {
      Some(settings_ctrl::updated_config(s, &services.config))
    } else {
      None
    };
    (task, cfg)
  };

  if let Some(cfg) = updated_cfg {
    let features = cfg.features();
    state.feat_asset_tracking = *features.asset_tracking();
    state.feat_mail = *features.mail();
    state.feat_skill_monitoring = *features.skill_monitoring();
    state.feat_wallet = *features.wallet();

    let hidden_nav = matches!(state.active_nav, Nav::Assets) && !state.feat_asset_tracking
      || matches!(state.active_nav, Nav::Mail) && !state.feat_mail
      || matches!(state.active_nav, Nav::Skills) && !state.feat_skill_monitoring
      || matches!(state.active_nav, Nav::Wallet) && !state.feat_wallet;

    if hidden_nav {
      state.active_nav = Nav::Characters;
      let (chars_state, chars_task) = characters_ctrl::new(state.characters.clone(), services);
      state.active_view = ActiveView::Characters(chars_state);
      let toast_task = iced::Task::done(Message::ShowToast("Preferences saved".to_string()));
      return (
        iced::Task::batch([settings_task, chars_task.map(Message::Characters), toast_task]),
        Some(cfg),
      );
    }

    let toast_task = iced::Task::done(Message::ShowToast("Preferences saved".to_string()));
    return (iced::Task::batch([settings_task, toast_task]), Some(cfg));
  }

  (settings_task, None)
}

fn update_skills(state: &mut State, msg: skills::Message, services: &Services) -> iced::Task<Message> {
  if let skills::Message::ReauthorizeCharacter(_) = &msg {
    return trigger_reauth(services);
  }
  let is_drag_end = matches!(&msg, skills::Message::PaneDragEnd);
  let ActiveView::Skills(s) = &mut state.active_view else {
    return iced::Task::none();
  };
  let task = skills_ctrl::update(s, msg, services).map(Message::Skills);
  if is_drag_end {
    state.skills_left_pane_width = s.left_pane_width;
  }
  task
}

fn update_sync_state(state: &mut State, had_updates: bool) {
  if state.sync.is_syncing() {
    if had_updates {
      state.refresh_successes += 1;
    }
    state.sync.complete_op();
    if !state.sync.is_syncing() {
      state.esi_connected = state.refresh_successes > 0;
      state.refresh_successes = 0;
    }
  }
  state.sync.record_background_sync();
}

fn update_wallet(state: &mut State, msg: wallet::Message, services: &Services) -> iced::Task<Message> {
  if let wallet::Message::ReauthorizeCharacter(_) = &msg {
    return trigger_reauth(services);
  }
  let is_drag_end = matches!(&msg, wallet::Message::PaneDragEnd);
  let corporations = state.corporations.clone();
  let ActiveView::Wallet(s) = &mut state.active_view else {
    return iced::Task::none();
  };
  let task = wallet_ctrl::update(s, msg, services, &corporations).map(Message::Wallet);
  if is_drag_end {
    state.wallet_right_rail_width = s.right_rail_width;
  }
  task
}

fn trigger_reauth(services: &Services) -> iced::Task<Message> {
  let Some(esi) = services.esi_client.clone() else {
    return iced::Task::none();
  };
  let scopes = services.config.features().required_scopes_for_character();
  let (url, verifier, oauth_state) = esi.auth().sign_in(&scopes, "http://127.0.0.1:47823/callback");
  let _ = open::that_detached(&url);
  let db = services.db.clone();
  iced::Task::perform(
    async move { characters_ctrl::reauthorize_character(esi, verifier, oauth_state, db).await },
    |result| match result {
      Ok(character) => Message::Characters(characters::Message::CharactersTab(
        pod_ui::views::characters::characters_tab::Message::CharacterAdded(character),
      )),
      Err(_) => Message::Characters(characters::Message::TagsApplied),
    },
  )
}
fn utc_time_string() -> String {
  let secs = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
  let h = (secs / 3600) % 24;
  let m = (secs / 60) % 60;
  let s = secs % 60;
  format!("{h:02}:{m:02}:{s:02}")
}
