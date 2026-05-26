//! Main window controller: navigation shell that routes to child view controllers.

use iced::{Subscription, widget::image};
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

/// Replaces a character in-place by ID and propagates the update to the active view.
pub fn apply_synced_character(state: &mut State, character: Character) {
  let Some(idx) = state.characters.iter().position(|c| *c.id() == *character.id()) else {
    return;
  };
  // Tags live in characters::State.all_characters when that view is active;
  // state.characters never has them loaded.
  let tags = match &state.active_view {
    ActiveView::Characters(s) => s
      .all_characters
      .iter()
      .find(|c| *c.id() == *character.id())
      .map(|c| c.tags().clone())
      .unwrap_or_default(),
    _ => state.characters[idx].tags().clone(),
  };
  let mut character = character;
  *character.tags_mut() = tags;
  state.characters[idx] = character.clone();
  let updated = state.characters.clone();
  match &mut state.active_view {
    ActiveView::Skills(s) => skills_ctrl::refresh_characters(s, updated),
    ActiveView::Characters(s) => {
      if let Some(bytes) = character.portrait_data() {
        s.character_pane
          .portrait_handles
          .insert(*character.id(), image::Handle::from_bytes(bytes.clone()));
      }
      if let Some(ci) = s.all_characters.iter().position(|c| *c.id() == *character.id()) {
        s.all_characters[ci] = character.clone();
      }
    }
    ActiveView::Assets(s) => {
      if let Some(bytes) = character.portrait_data() {
        s.abyssals
          .portrait_handles
          .insert(*character.id(), image::Handle::from_bytes(bytes.clone()));
      }
    }
    _ => {}
  }
  if let Some(cached) = state.cached_assets_state.as_mut()
    && let Some(bytes) = character.portrait_data()
  {
    cached
      .abyssals
      .portrait_handles
      .insert(*character.id(), image::Handle::from_bytes(bytes.clone()));
  }
}

/// Spawns a background asset reload to keep `cached_assets_state` fresh.
///
/// Called after a character sync so the next Assets restore shows current data.
/// Returns `Task::none()` when no cache exists or assets is already active.
pub fn refresh_cached_assets_if_needed(state: &State, services: &Services) -> iced::Task<Message> {
  if state.cached_assets_state.is_none() || matches!(state.active_view, ActiveView::Assets(_)) {
    return iced::Task::none();
  }
  assets_ctrl::background_reload(state.characters.clone(), state.corporations.clone(), services).map(Message::Assets)
}

/// Creates a new shell state and a startup task that initializes the default view.
pub fn new(
  characters: Vec<Character>,
  services: &Services,
  skills_left_pane_width: Option<f32>,
  mail_folder_pane_width: Option<f32>,
  mail_message_list_width: Option<f32>,
  wallet_right_rail_width: Option<f32>,
  assets_sidebar_width: Option<f32>,
  abyssals_filter_pane_width: Option<f32>,
) -> (State, iced::Task<Message>) {
  let features = services.config.features();
  let (chars_state, chars_task) = characters_ctrl::new(characters.clone(), services);
  let state = State {
    abyssals_filter_pane_width: abyssals_filter_pane_width.unwrap_or(220.0),
    active_nav: Nav::Characters,
    active_view: ActiveView::Characters(chars_state),
    assets_sidebar_width: assets_sidebar_width.unwrap_or(232.0),
    cached_assets_state: None,
    cached_wallet_state: None,
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
    mail_nav: pod_ui::views::main_window::MailNavState::default(),
    pending_snooze_expired: Vec::new(),
    refresh_successes: 0,
    registry_has_server_error: false,
    registry_in_flight: Vec::new(),
    registry_last_synced_at: None,
    skills_left_pane_width: skills_left_pane_width.unwrap_or(700.0),
    skills_nav: pod_ui::views::main_window::SkillsNavState::default(),
    sync: status_bar::SyncState::default(),
    toast: None,
    wallet_right_rail_width: wallet_right_rail_width.unwrap_or(220.0),
  };
  (state, chars_task.map(Message::Characters))
}

/// Opens the OAuth re-authorization flow for a character with an expired or missing token.
pub fn reauth(services: &Services) -> iced::Task<Message> {
  trigger_reauth(services)
}

fn subscription_for_streaming_views(state: &State) -> Subscription<Message> {
  match &state.active_view {
    ActiveView::Skills(s) => skills_ctrl::subscription(s).map(Message::Skills),
    ActiveView::Wallet(s) => wallet::subscription(s).map(Message::Wallet),
    _ => Subscription::none(),
  }
}

fn view_subscription(state: &State) -> Subscription<Message> {
  match &state.active_view {
    ActiveView::Assets(s) => assets::subscription(s).map(Message::Assets),
    ActiveView::Characters(s) => characters_ctrl::subscription(s).map(Message::Characters),
    ActiveView::Mail(s) => mail::subscription(s).map(Message::Mail),
    _ => subscription_for_streaming_views(state),
  }
}

/// Returns background subscriptions for the currently active view plus
/// a global snooze-expiry timer.
pub fn subscription(state: &State) -> Subscription<Message> {
  let view_sub = view_subscription(state);
  if state.feat_mail {
    let snooze_sub = iced::time::every(std::time::Duration::from_secs(60)).map(|_| Message::SnoozeTick);
    Subscription::batch([view_sub, snooze_sub])
  } else {
    view_sub
  }
}

/// Processes a main window message and returns a task.
///
/// Returns `(task, Option<new_config>, Option<restart_msg>)` — the config
/// is `Some` whenever a settings toggle or reset just ran, so the caller
/// can update `app.config`; the restart_msg is `Some` when a storage path
/// change requires a restart.
pub fn update(
  state: &mut State,
  message: Message,
  services: &Services,
) -> (iced::Task<Message>, Option<crate::config::Settings>, Option<String>) {
  if let Some(result) = update_state_only(state, &message) {
    return result;
  }
  match message {
    Message::Assets(msg) => (update_assets(state, msg, services), None, None),
    Message::Characters(msg) => (update_characters(state, msg, services), None, None),
    msg => update_settings_or_other(state, msg, services),
  }
}

fn update_settings_or_other(
  state: &mut State,
  msg: Message,
  services: &Services,
) -> (iced::Task<Message>, Option<crate::config::Settings>, Option<String>) {
  match msg {
    Message::Settings(msg) => update_settings(state, msg, services),
    msg => handle_other_message(state, msg, services),
  }
}

fn handle_nav_utility_message(
  state: &mut State,
  msg: Message,
  services: &Services,
) -> (iced::Task<Message>, Option<crate::config::Settings>, Option<String>) {
  match msg {
    Message::Navigate(nav) => (update_navigate(state, nav, services), None, None),
    Message::RefreshAll | Message::StatusBar(status_bar::Message::RefreshPressed) => {
      (update_refresh_all(state, services), None, None)
    }
    Message::ShowToast(msg) => (show_toast(state, msg), None, None),
    msg => handle_snooze_or_ignore(state, msg, services),
  }
}

fn handle_snooze_or_ignore(
  state: &mut State,
  msg: Message,
  services: &Services,
) -> (iced::Task<Message>, Option<crate::config::Settings>, Option<String>) {
  match msg {
    Message::SnoozeTick => (update_snooze_tick(state, services), None, None),
    _ => (iced::Task::none(), None, None),
  }
}

fn handle_other_message(
  state: &mut State,
  msg: Message,
  services: &Services,
) -> (iced::Task<Message>, Option<crate::config::Settings>, Option<String>) {
  match msg {
    Message::CharacterDetail(m) => (update_character_detail(state, m, services), None, None),
    Message::Mail(m) => (update_mail(state, m, services), None, None),
    msg => handle_skills_wallet_or_nav(state, msg, services),
  }
}

fn handle_skills_wallet_or_nav(
  state: &mut State,
  msg: Message,
  services: &Services,
) -> (iced::Task<Message>, Option<crate::config::Settings>, Option<String>) {
  match msg {
    Message::Skills(m) => (update_skills(state, m, services), None, None),
    Message::Wallet(m) => (update_wallet(state, m, services), None, None),
    msg => handle_nav_utility_message(state, msg, services),
  }
}

fn update_state_only(
  state: &mut State,
  message: &Message,
) -> Option<(iced::Task<Message>, Option<crate::config::Settings>, Option<String>)> {
  match message {
    Message::DismissToast => {
      state.toast = None;
      Some((iced::Task::none(), None, None))
    }
    Message::EveTimeTick => {
      state.eve_time = utc_time_string();
      Some((iced::Task::none(), None, None))
    }
    Message::HoverNav(nav) => {
      state.hovered_nav = *nav;
      Some((iced::Task::none(), None, None))
    }
    _ => None,
  }
}

fn show_toast(state: &mut State, msg: String) -> iced::Task<Message> {
  state.toast = Some(msg);
  iced::Task::perform(
    async { tokio::time::sleep(std::time::Duration::from_millis(2500)).await },
    |()| Message::DismissToast,
  )
}

fn apply_assets_loaded_to_cache(state: &mut State, result: &Result<Vec<pod_ui::views::assets::AssetRecord>, String>) {
  if let Some(cached) = state.cached_assets_state.as_mut() {
    if let Ok(records) = result {
      cached.assets = records.clone();
    }
    cached.loading = false;
  }
}

fn try_handle_assets_early_exit(
  state: &mut State,
  msg: &assets::Message,
  services: &Services,
) -> Option<iced::Task<Message>> {
  if let assets::Message::ReauthorizeCharacter(_) = msg {
    return Some(trigger_reauth(services));
  }
  // When assets is not the active view, an AssetsLoaded from a background
  // refresh should update the cache rather than be discarded.
  if let assets::Message::AssetsLoaded(result) = msg
    && !matches!(state.active_view, ActiveView::Assets(_))
  {
    apply_assets_loaded_to_cache(state, result);
    return Some(iced::Task::none());
  }
  None
}

fn save_sidebar_width_if_drag_end(state: &mut State, is_drag_end: bool) {
  if !is_drag_end {
    return;
  }
  if let ActiveView::Assets(s) = &state.active_view {
    state.assets_sidebar_width = s.sidebar_width;
  }
}

fn save_abyssals_filter_pane_width_if_drag_end(state: &mut State, is_drag_end: bool) {
  if !is_drag_end {
    return;
  }
  if let ActiveView::Assets(s) = &state.active_view {
    state.abyssals_filter_pane_width = s.abyssals.filter_pane_width;
  }
}

fn update_assets(state: &mut State, msg: assets::Message, services: &Services) -> iced::Task<Message> {
  if let Some(task) = try_handle_assets_early_exit(state, &msg, services) {
    return task;
  }
  let is_drag_end = matches!(&msg, assets::Message::PaneDragEnd);
  let is_abyssals_drag_end = matches!(
    &msg,
    assets::Message::AbyssalsTab(pod_ui::views::assets::abyssals_tab::Message::PaneDragEnd)
  );
  let chars_snapshot = state.characters.clone();
  let ActiveView::Assets(s) = &mut state.active_view else {
    return iced::Task::none();
  };
  if let assets::Message::RefreshNavHistory = &msg {
    return handle_assets_refresh_nav_history(s, services);
  }
  let should_refresh_values = should_assets_refresh_values(&msg, s);
  let new_items = collect_new_item_icons(&msg, s);
  if let assets::Message::StockpilesTab(assets::stockpiles_tab::Message::FormSave) = &msg {
    return handle_assets_stockpile_save(s, msg, services);
  }
  if let assets::Message::StockpilesTab(assets::stockpiles_tab::Message::DeleteStockpile(id)) = &msg {
    let id = *id;
    return handle_assets_stockpile_delete(s, msg, id, services);
  }
  let base_task = assets::update(s, msg).map(Message::Assets);
  let task = build_assets_follow_up_tasks(base_task, should_refresh_values, new_items, s, chars_snapshot, services);
  save_sidebar_width_if_drag_end(state, is_drag_end);
  save_abyssals_filter_pane_width_if_drag_end(state, is_abyssals_drag_end);
  task
}

fn handle_assets_refresh_nav_history(s: &mut assets::State, services: &Services) -> iced::Task<Message> {
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
  iced::Task::none()
}

fn should_assets_refresh_values(msg: &assets::Message, s: &assets::State) -> bool {
  matches!(msg, assets::Message::TabSelected(assets::Tab::Values))
    || (matches!(msg, assets::Message::AssetsLoaded(_)) && s.active_tab == assets::Tab::Values)
}

fn extract_missing_icons(records: &[pod_ui::views::assets::AssetRecord], s: &assets::State) -> Vec<(i32, String)> {
  records
    .iter()
    .map(|r| (r.type_id, r.icon_variant.clone()))
    .collect::<std::collections::HashSet<_>>()
    .into_iter()
    .filter(|(type_id, variant)| !s.item_icons.contains_key(&(*type_id, variant.clone())))
    .collect()
}

fn collect_new_item_icons(msg: &assets::Message, s: &assets::State) -> Option<Vec<(i32, String)>> {
  let assets::Message::AssetsLoaded(Ok(records)) = msg else {
    return None;
  };
  let items = extract_missing_icons(records, s);
  if items.is_empty() { None } else { Some(items) }
}

fn parse_stockpile_items(form: &assets::StockpileForm) -> Vec<(i32, i32)> {
  form
    .items
    .iter()
    .filter_map(|it| {
      let t = it.type_id_text.trim().parse::<i32>().ok()?;
      let q = it.qty_text.trim().parse::<i32>().ok()?;
      Some((t, q))
    })
    .collect()
}

fn build_stockpile_db_task(db: pod_db::Repo, form: &assets::StockpileForm) -> iced::Task<Message> {
  let name = form.name.clone();
  let location_id = form.location_id_text.trim().parse::<i64>().ok();
  let items = parse_stockpile_items(form);
  if let Some(editing_id) = form.editing_id {
    iced::Task::perform(
      assets_ctrl::update_stockpile(db, editing_id, name, location_id, None, items),
      |piles| Message::Assets(assets::Message::StockpilesLoaded(piles)),
    )
  } else {
    iced::Task::perform(
      assets_ctrl::create_stockpile(db, name, location_id, None, items),
      |piles| Message::Assets(assets::Message::StockpilesLoaded(piles)),
    )
  }
}

fn handle_assets_stockpile_save(
  s: &mut assets::State,
  msg: assets::Message,
  services: &Services,
) -> iced::Task<Message> {
  let Some(form) = s.stockpile_form.clone() else {
    return iced::Task::none();
  };
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  let base_task = assets::update(s, msg).map(Message::Assets);
  let db_task = build_stockpile_db_task(db, &form);
  s.stockpile_form = None;
  iced::Task::batch([base_task, db_task])
}

fn handle_assets_stockpile_delete(
  s: &mut assets::State,
  msg: assets::Message,
  id: i64,
  services: &Services,
) -> iced::Task<Message> {
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  let base_task = assets::update(s, msg).map(Message::Assets);
  let delete_task = iced::Task::perform(assets_ctrl::delete_stockpile(db, id), |piles| {
    Message::Assets(assets::Message::StockpilesLoaded(piles))
  });
  iced::Task::batch([base_task, delete_task])
}

fn build_values_task(
  should_refresh: bool,
  s: &mut assets::State,
  chars_snapshot: Vec<Character>,
) -> Option<iced::Task<Message>> {
  if !should_refresh {
    return None;
  }
  let assets_snapshot = s.assets.clone();
  s.values_loading = true;
  Some(iced::Task::perform(
    assets_ctrl::asset_values_breakdown(assets_snapshot, chars_snapshot),
    |data| Message::Assets(assets::Message::ValuesLoaded(data)),
  ))
}

fn build_icon_task(new_items: Option<Vec<(i32, String)>>, services: &Services) -> Option<iced::Task<Message>> {
  let (items, esi, db) = (new_items?, services.esi_client.clone()?, services.db.clone()?);
  Some(iced::Task::perform(
    async move { assets_ctrl::fetch_type_icons(items, esi, db).await },
    |icons| Message::Assets(assets::Message::ItemIconsLoaded(icons)),
  ))
}

fn build_assets_follow_up_tasks(
  base_task: iced::Task<Message>,
  should_refresh_values: bool,
  new_items: Option<Vec<(i32, String)>>,
  s: &mut assets::State,
  chars_snapshot: Vec<Character>,
  services: &Services,
) -> iced::Task<Message> {
  let values_task = build_values_task(should_refresh_values, s, chars_snapshot);
  let icon_task = build_icon_task(new_items, services);
  let mut tasks = vec![base_task];
  if let Some(vt) = values_task {
    tasks.push(vt);
  }
  if let Some(it) = icon_task {
    tasks.push(it);
  }
  iced::Task::batch(tasks)
}

fn extract_character_detail_nav_id(msg: &character_detail::Message) -> Option<i64> {
  match msg {
    character_detail::Message::CharacterSwitched(id) => Some(*id),
    character_detail::Message::Picker(character_picker::Message::Select(
      character_picker::PickerSelection::Character(id),
    )) => Some(*id),
    _ => None,
  }
}

fn navigate_to_character_detail(state: &mut State, char_id: i64, services: &Services) -> iced::Task<Message> {
  let Some(character) = state.characters.iter().find(|c| *c.id() == char_id).cloned() else {
    return iced::Task::none();
  };
  let (detail_state, detail_task) = character_detail_ctrl::new(char_id, character, state.characters.clone(), services);
  state.active_view = ActiveView::CharacterDetail(detail_state);
  detail_task.map(Message::CharacterDetail)
}

fn update_character_detail(
  state: &mut State,
  msg: character_detail::Message,
  services: &Services,
) -> iced::Task<Message> {
  if let Some(char_id) = extract_character_detail_nav_id(&msg) {
    return navigate_to_character_detail(state, char_id, services);
  }
  if let character_detail::Message::ReauthorizeCharacter(_) = &msg {
    return trigger_reauth(services);
  }
  let ActiveView::CharacterDetail(s) = &mut state.active_view else {
    return iced::Task::none();
  };
  character_detail_ctrl::update(s, msg, services).map(Message::CharacterDetail)
}

fn is_characters_drag_end(msg: &characters::Message) -> bool {
  matches!(
    msg,
    characters::Message::CharactersTab(m)
      if matches!(m, characters::characters_tab::Message::DragEnd)
  )
}

fn sync_character_order_after_drag(state: &mut State) {
  let ActiveView::Characters(s) = &state.active_view else {
    return;
  };
  let new_order: Vec<i64> = s.all_characters.iter().map(|c| *c.id()).collect();
  state
    .characters
    .sort_by_key(|c| new_order.iter().position(|&id| id == *c.id()).unwrap_or(usize::MAX));
}

fn update_characters(state: &mut State, msg: characters::Message, services: &Services) -> iced::Task<Message> {
  let is_drag_end = is_characters_drag_end(&msg);
  if let Some(task) = handle_characters_navigation(state, &msg, services) {
    return task;
  }
  apply_characters_state_updates(state, &msg);
  let ActiveView::Characters(chars_state) = &mut state.active_view else {
    return iced::Task::none();
  };
  let task = characters_ctrl::update(chars_state, msg, services).map(Message::Characters);
  if is_drag_end {
    sync_character_order_after_drag(state);
  }
  task
}

fn nav_to_character_detail_from_list(
  state: &mut State,
  char_id: i64,
  services: &Services,
) -> Option<iced::Task<Message>> {
  let character = state.characters.iter().find(|c| *c.id() == char_id).cloned()?;
  state.active_nav = Nav::Characters;
  let (detail_state, detail_task) = character_detail_ctrl::new(char_id, character, state.characters.clone(), services);
  state.active_view = ActiveView::CharacterDetail(detail_state);
  Some(detail_task.map(Message::CharacterDetail))
}

fn nav_to_skills_for_character(state: &mut State, char_id: i64, services: &Services) -> Option<iced::Task<Message>> {
  state.active_nav = Nav::Skills;
  let mut s = skills_ctrl::new(
    state.characters.clone(),
    state.skills_left_pane_width,
    Some(char_id),
    services,
  );
  let _ = skills_ctrl::update(
    &mut s,
    skills::Message::Picker(character_picker::Message::Select(
      character_picker::PickerSelection::Character(char_id),
    )),
    services,
  );
  state.active_view = ActiveView::Skills(s);
  Some(iced::Task::none())
}

fn nav_to_wallet_for_character(state: &mut State, char_id: i64, services: &Services) -> Option<iced::Task<Message>> {
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
  Some(task.map(Message::Wallet))
}

fn handle_characters_navigation(
  state: &mut State,
  msg: &characters::Message,
  services: &Services,
) -> Option<iced::Task<Message>> {
  match msg {
    characters::Message::CharactersTab(characters::characters_tab::Message::NavigateToDetail(id)) => {
      nav_to_character_detail_from_list(state, *id, services)
    }
    characters::Message::CharactersTab(characters::characters_tab::Message::NavigateToSkills(id)) => {
      nav_to_skills_for_character(state, *id, services)
    }
    characters::Message::CharactersTab(characters::characters_tab::Message::NavigateToWallet(id)) => {
      nav_to_wallet_for_character(state, *id, services)
    }
    _ => None,
  }
}

fn apply_characters_tab_update(state: &mut State, tab_msg: &characters::characters_tab::Message) {
  match tab_msg {
    characters::characters_tab::Message::CharacterAdded(c) => apply_character_added(state, c),
    characters::characters_tab::Message::SkillQueuesRefreshed(updates) => {
      apply_skill_queues_refreshed(state, updates);
    }
    tab_msg => apply_characters_tab_sync_update(state, tab_msg),
  }
}

fn apply_characters_tab_sync_update(state: &mut State, tab_msg: &characters::characters_tab::Message) {
  match tab_msg {
    characters::characters_tab::Message::LocationsRefreshed(updates) => {
      update_sync_state(state, !updates.is_empty());
    }
    characters::characters_tab::Message::WalletsRefreshed(updates) => {
      update_sync_state(state, !updates.is_empty());
    }
    _ => {}
  }
}

fn apply_characters_state_updates(state: &mut State, msg: &characters::Message) {
  match msg {
    characters::Message::CharactersTab(tab_msg) => apply_characters_tab_update(state, tab_msg),
    characters::Message::CorporationsTab(tab_msg) => apply_corporations_tab_update(state, tab_msg),
    msg => apply_characters_removal_updates(state, msg),
  }
}

fn apply_characters_removal_updates(state: &mut State, msg: &characters::Message) {
  match msg {
    characters::Message::ConfirmRemove => apply_confirm_remove(state),
    characters::Message::ConfirmRemoveCorporation => apply_confirm_remove_corporation(state),
    _ => {}
  }
}

fn apply_character_added(state: &mut State, c: &Character) {
  if !state.characters.iter().any(|existing| existing.id() == c.id()) {
    state.characters.push(c.clone());
  }
}

fn apply_confirm_remove(state: &mut State) {
  if let ActiveView::Characters(s) = &state.active_view
    && let Some(id) = s.confirm_remove
  {
    state.characters.retain(|c| *c.id() != id);
  }
}

fn apply_confirm_remove_corporation(state: &mut State) {
  if let ActiveView::Characters(s) = &state.active_view
    && let Some(id) = s.confirm_remove_corporation
  {
    state.corporations.retain(|c: &Corporation| *c.id() != id);
  }
}

fn apply_corporation_added(state: &mut State, corp: &Corporation) {
  state.corporations.retain(|c: &Corporation| *c.id() != *corp.id());
  state.corporations.push(corp.clone());
}

fn remove_corporation_by_id(state: &mut State, id: &i64) {
  state.corporations.retain(|c: &Corporation| *c.id() != *id);
}

fn apply_corporations_tab_update(state: &mut State, tab_msg: &characters::corporations_tab::Message) {
  match tab_msg {
    characters::corporations_tab::Message::CorporationAdded(corp) => apply_corporation_added(state, corp),
    characters::corporations_tab::Message::CorporationRemoved(id) => remove_corporation_by_id(state, id),
    characters::corporations_tab::Message::CorporationsLoaded(corps) => {
      state.corporations = corps.clone();
    }
    _ => {}
  }
}

fn apply_character_skill_update(c: &mut Character, skills: &[CharacterSkill], tq: &[pod_model::TrainingQueueEntry]) {
  let skill_map: std::collections::HashMap<i32, &CharacterSkill> = skills.iter().map(|s| (s.skill_id, s)).collect();
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
  *c.training_queue_mut() = tq.to_vec();
}

fn apply_skill_queues_refreshed(
  state: &mut State,
  updates: &[(i64, Vec<CharacterSkill>, Vec<pod_model::TrainingQueueEntry>)],
) {
  update_sync_state(state, !updates.is_empty());
  for (id, skills, tq) in updates {
    let Some(c) = state.characters.iter_mut().find(|c| *c.id() == *id) else {
      continue;
    };
    apply_character_skill_update(c, skills, tq);
  }
  if let ActiveView::Skills(s) = &mut state.active_view {
    let updated_chars = state.characters.clone();
    skills_ctrl::refresh_characters(s, updated_chars);
  }
}

fn update_mail(state: &mut State, msg: mail::Message, services: &Services) -> iced::Task<Message> {
  if let mail::Message::ReauthorizeCharacter(_) = &msg {
    return trigger_reauth(services);
  }
  // When snoozes expire while mail is inactive, queue the pairs for
  // application the next time the mail view becomes active.
  if let mail::Message::ReadingPane(pod_ui::views::mail::reading_pane::Message::SnoozedExpired(ref pairs)) = msg
    && !matches!(state.active_view, ActiveView::Mail(_))
  {
    state.pending_snooze_expired.extend_from_slice(pairs);
    return iced::Task::none();
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

fn navigate_to_assets(state: &mut State, services: &Services) -> iced::Task<Message> {
  if let Some(cached) = state.cached_assets_state.take() {
    let refresh = assets_ctrl::background_reload(state.characters.clone(), state.corporations.clone(), services);
    state.active_view = ActiveView::Assets(cached);
    return refresh.map(Message::Assets);
  }
  let (s, task) = assets_ctrl::new(
    state.characters.clone(),
    state.corporations.clone(),
    services,
    state.assets_sidebar_width,
    state.abyssals_filter_pane_width,
  );
  state.active_view = ActiveView::Assets(s);
  task.map(Message::Assets)
}

fn navigate_to_mail(state: &mut State, services: &Services) -> iced::Task<Message> {
  let nav = state.mail_nav.clone();
  let (mut s, task) = mail_ctrl::new(
    state.characters.clone(),
    services,
    state.mail_folder_pane_width,
    state.mail_message_list_width,
  );
  mail_ctrl::restore_nav_state(&mut s, &nav);
  swap_active_view(state, ActiveView::Mail(s));
  let pending = std::mem::take(&mut state.pending_snooze_expired);
  if pending.is_empty() {
    return task.map(Message::Mail);
  }
  let flush = Message::Mail(mail::Message::ReadingPane(
    pod_ui::views::mail::reading_pane::Message::SnoozedExpired(pending),
  ));
  iced::Task::batch([task.map(Message::Mail), iced::Task::done(flush)])
}

fn navigate_to_characters(state: &mut State, services: &Services) -> iced::Task<Message> {
  let (chars_state, init_task) = characters_ctrl::new(state.characters.clone(), services);
  swap_active_view(state, ActiveView::Characters(chars_state));
  let tags_task = characters_ctrl::reload_all_tags_task(services).map(Message::Characters);
  iced::Task::batch([init_task.map(Message::Characters), tags_task])
}

fn navigate_to_simple_view(state: &mut State, nav: Nav, services: &Services) -> iced::Task<Message> {
  match nav {
    Nav::Settings => {
      let (s, task) = settings_ctrl::new(&services.config);
      swap_active_view(state, ActiveView::Settings(s));
      task.map(Message::Settings)
    }
    Nav::Skills => {
      let nav = state.skills_nav.clone();
      let mut s = skills_ctrl::new(
        state.characters.clone(),
        state.skills_left_pane_width,
        Some(nav.selected_char_id),
        services,
      );
      skills_ctrl::apply_nav_state(&mut s, &nav);
      swap_active_view(state, ActiveView::Skills(s));
      iced::Task::none()
    }
    Nav::Wallet => navigate_to_wallet(state, services),
    _ => iced::Task::none(),
  }
}

fn navigate_to_wallet(state: &mut State, services: &Services) -> iced::Task<Message> {
  if let Some(cached) = state.cached_wallet_state.take() {
    swap_active_view(state, ActiveView::Wallet(cached));
    return iced::Task::none();
  }
  let (s, task) = wallet_ctrl::new(
    state.characters.clone(),
    state.corporations.clone(),
    services,
    state.wallet_right_rail_width,
  );
  swap_active_view(state, ActiveView::Wallet(s));
  task.map(Message::Wallet)
}

fn update_navigate(state: &mut State, nav: Nav, services: &Services) -> iced::Task<Message> {
  tracing::info!("main: navigated to {nav:?}");
  state.active_nav = nav;
  match nav {
    Nav::Assets => navigate_to_assets(state, services),
    Nav::Characters => navigate_to_characters(state, services),
    Nav::Mail => navigate_to_mail(state, services),
    nav => navigate_to_simple_view(state, nav, services),
  }
}

/// Replaces `state.active_view` with `new_view`; saves the prior view's
/// nav state (assets cache, mail nav, skills nav, wallet cache) before
/// replacing.
fn swap_active_view(state: &mut State, new_view: ActiveView) {
  let prev = std::mem::replace(&mut state.active_view, new_view);
  match prev {
    ActiveView::Assets(s) => {
      state.cached_assets_state = Some(s);
    }
    ActiveView::Mail(s) => {
      state.mail_nav = pod_ui::views::main_window::MailNavState {
        selected_folder: Some(s.selected_folder),
        selected_message_id: s.selected_message_id,
      };
    }
    ActiveView::Skills(s) => {
      state.skills_nav = pod_ui::views::skills::NavState::from_state(&s);
      state.skills_left_pane_width = s.left_pane_width;
    }
    ActiveView::Wallet(s) => {
      state.cached_wallet_state = Some(s);
    }
    _ => {}
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
) -> (iced::Task<Message>, Option<crate::config::Settings>, Option<String>) {
  let is_features_save = matches!(
    &msg,
    settings::Message::FeaturesTab(settings::features_tab::Message::ToggleFeature(_))
      | settings::Message::ResetDefaults
  );
  let ActiveView::Settings(s) = &mut state.active_view else {
    return (iced::Task::none(), None, None);
  };
  let (settings_task, restart_msg, storage_cfg) = settings_ctrl::update(s, msg, services);
  let settings_task = settings_task.map(Message::Settings);
  let updated_cfg = if is_features_save {
    Some(settings_ctrl::updated_config(s, &services.config))
  } else {
    storage_cfg
  };

  apply_settings_save(state, settings_task, updated_cfg, restart_msg, services)
}

fn nav_feature_flag(state: &State, nav: Nav) -> Option<bool> {
  match nav {
    Nav::Assets => Some(state.feat_asset_tracking),
    Nav::Mail => Some(state.feat_mail),
    nav => nav_feature_flag_ext(state, nav),
  }
}

fn nav_feature_flag_ext(state: &State, nav: Nav) -> Option<bool> {
  match nav {
    Nav::Skills => Some(state.feat_skill_monitoring),
    Nav::Wallet => Some(state.feat_wallet),
    _ => None,
  }
}

fn nav_feature_enabled(state: &State, nav: Nav) -> bool {
  nav_feature_flag(state, nav).unwrap_or(true)
}

fn active_nav_was_hidden(state: &State) -> bool {
  !nav_feature_enabled(state, state.active_nav)
}

fn apply_settings_features(state: &mut State, cfg: &crate::config::Settings) {
  let features = cfg.features();
  state.feat_asset_tracking = *features.asset_tracking();
  state.feat_mail = *features.mail();
  state.feat_skill_monitoring = *features.skill_monitoring();
  state.feat_wallet = *features.wallet();
}

fn apply_settings_save(
  state: &mut State,
  settings_task: iced::Task<Message>,
  updated_cfg: Option<crate::config::Settings>,
  restart_msg: Option<String>,
  services: &Services,
) -> (iced::Task<Message>, Option<crate::config::Settings>, Option<String>) {
  let Some(cfg) = updated_cfg else {
    return (settings_task, None, restart_msg);
  };
  apply_settings_features(state, &cfg);
  let toast_task = iced::Task::done(Message::ShowToast("Preferences saved".to_string()));
  if active_nav_was_hidden(state) {
    state.active_nav = Nav::Characters;
    let (chars_state, chars_task) = characters_ctrl::new(state.characters.clone(), services);
    state.active_view = ActiveView::Characters(chars_state);
    return (
      iced::Task::batch([settings_task, chars_task.map(Message::Characters), toast_task]),
      Some(cfg),
      restart_msg,
    );
  }
  (iced::Task::batch([settings_task, toast_task]), Some(cfg), restart_msg)
}

fn update_snooze_tick(state: &mut State, services: &Services) -> iced::Task<Message> {
  mail_ctrl::snooze_tick_task(state.characters.clone(), services).map(Message::Mail)
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
  tracing::info!("auth: opening browser for OAuth (reauthorize character)");
  if let Err(e) = open::that_detached(&url) {
    tracing::warn!("auth: failed to open browser: {e}");
  }
  tracing::info!("auth: waiting for OAuth callback on port 47823");
  let db = services.db.clone();
  let oauth_tx = services.oauth_callback_tx.clone();
  iced::Task::perform(
    async move { characters_ctrl::reauthorize_character(esi, oauth_tx, verifier, oauth_state, db).await },
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

#[cfg(test)]
mod tests {
  use super::*;

  mod apply_synced_character {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_state(character: Character) -> State {
      State {
        abyssals_filter_pane_width: 220.0,
        active_nav: Nav::Settings,
        active_view: ActiveView::Settings(settings::State::default()),
        assets_sidebar_width: 232.0,
        cached_assets_state: None,
        cached_wallet_state: None,
        characters: vec![character],
        corporations: Vec::new(),
        esi_connected: false,
        eve_time: String::new(),
        feat_asset_tracking: false,
        feat_mail: false,
        feat_skill_monitoring: false,
        feat_wallet: false,
        hovered_nav: None,
        mail_folder_pane_width: 0.0,
        mail_message_list_width: 0.0,
        mail_nav: pod_ui::views::main_window::MailNavState::default(),
        pending_snooze_expired: Vec::new(),
        refresh_successes: 0,
        registry_has_server_error: false,
        registry_in_flight: Vec::new(),
        registry_last_synced_at: None,
        skills_left_pane_width: 0.0,
        skills_nav: Default::default(),
        sync: status_bar::SyncState::default(),
        toast: None,
        wallet_right_rail_width: 0.0,
      }
    }

    #[test]
    fn it_does_nothing_if_character_id_not_found() {
      let existing = Character::new(1, "Alpha");
      let mut state = make_state(existing);
      let synced = Character::new(999, "Ghost");

      apply_synced_character(&mut state, synced);

      assert_eq!(state.characters.len(), 1);
      assert_eq!(*state.characters[0].id(), 1);
    }

    #[test]
    fn it_preserves_tags_from_the_existing_character() {
      let mut existing = Character::new(1, "Alpha");
      *existing.tags_mut() = vec![(1, "pvp".to_string(), None), (2, "trader".to_string(), None)];
      let mut state = make_state(existing);
      let synced = Character::new(1, "Alpha");

      apply_synced_character(&mut state, synced);

      assert_eq!(
        state.characters[0].tags(),
        &vec![(1, "pvp".to_string(), None), (2, "trader".to_string(), None)]
      );
    }

    #[test]
    fn it_preserves_tags_from_all_characters_when_characters_view_is_active() {
      use pod_ui::views::characters::State as CharactersState;

      let mut existing = Character::new(1, "Alpha");
      *existing.tags_mut() = vec![(1, "pvp".to_string(), None), (2, "trader".to_string(), None)];
      let chars_view = CharactersState::new(vec![existing]);
      let mut state = State {
        abyssals_filter_pane_width: 220.0,
        active_nav: Nav::Characters,
        active_view: ActiveView::Characters(chars_view),
        assets_sidebar_width: 232.0,
        cached_assets_state: None,
        cached_wallet_state: None,
        characters: vec![Character::new(1, "Alpha")],
        corporations: Vec::new(),
        esi_connected: false,
        eve_time: String::new(),
        feat_asset_tracking: false,
        feat_mail: false,
        feat_skill_monitoring: false,
        feat_wallet: false,
        hovered_nav: None,
        mail_folder_pane_width: 0.0,
        mail_message_list_width: 0.0,
        mail_nav: pod_ui::views::main_window::MailNavState::default(),
        pending_snooze_expired: Vec::new(),
        refresh_successes: 0,
        registry_has_server_error: false,
        registry_in_flight: Vec::new(),
        registry_last_synced_at: None,
        skills_left_pane_width: 0.0,
        skills_nav: Default::default(),
        sync: status_bar::SyncState::default(),
        toast: None,
        wallet_right_rail_width: 0.0,
      };
      let synced = Character::new(1, "Alpha");

      apply_synced_character(&mut state, synced);

      assert_eq!(
        state.characters[0].tags(),
        &vec![(1, "pvp".to_string(), None), (2, "trader".to_string(), None)]
      );
    }
  }

  mod apply_assets_loaded_to_cache {
    use super::*;

    fn make_empty_assets_state() -> assets::State {
      assets::new(vec![], vec![], 232.0, 220.0)
    }

    fn make_main_state_with_cache(cached: assets::State) -> State {
      State {
        abyssals_filter_pane_width: 220.0,
        active_nav: Nav::Settings,
        active_view: ActiveView::Settings(settings::State::default()),
        assets_sidebar_width: 232.0,
        cached_assets_state: Some(cached),
        cached_wallet_state: None,
        characters: Vec::new(),
        corporations: Vec::new(),
        esi_connected: false,
        eve_time: String::new(),
        feat_asset_tracking: false,
        feat_mail: false,
        feat_skill_monitoring: false,
        feat_wallet: false,
        hovered_nav: None,
        mail_folder_pane_width: 0.0,
        mail_message_list_width: 0.0,
        mail_nav: pod_ui::views::main_window::MailNavState::default(),
        pending_snooze_expired: Vec::new(),
        refresh_successes: 0,
        registry_has_server_error: false,
        registry_in_flight: Vec::new(),
        registry_last_synced_at: None,
        skills_left_pane_width: 0.0,
        skills_nav: Default::default(),
        sync: status_bar::SyncState::default(),
        toast: None,
        wallet_right_rail_width: 0.0,
      }
    }

    #[test]
    fn it_clears_loading_flag_on_ok_result() {
      let mut cached = make_empty_assets_state();
      cached.loading = true;
      let mut state = make_main_state_with_cache(cached);

      apply_assets_loaded_to_cache(&mut state, &Ok(vec![]));

      assert!(!state.cached_assets_state.as_ref().unwrap().loading);
    }

    #[test]
    fn it_clears_loading_flag_on_err_result() {
      let mut cached = make_empty_assets_state();
      cached.loading = true;
      let mut state = make_main_state_with_cache(cached);

      apply_assets_loaded_to_cache(&mut state, &Err("db error".to_string()));

      assert!(!state.cached_assets_state.as_ref().unwrap().loading);
    }

    #[test]
    fn it_is_noop_when_no_cached_state() {
      let mut state = State {
        abyssals_filter_pane_width: 220.0,
        active_nav: Nav::Settings,
        active_view: ActiveView::Settings(settings::State::default()),
        assets_sidebar_width: 232.0,
        cached_assets_state: None,
        cached_wallet_state: None,
        characters: Vec::new(),
        corporations: Vec::new(),
        esi_connected: false,
        eve_time: String::new(),
        feat_asset_tracking: false,
        feat_mail: false,
        feat_skill_monitoring: false,
        feat_wallet: false,
        hovered_nav: None,
        mail_folder_pane_width: 0.0,
        mail_message_list_width: 0.0,
        mail_nav: pod_ui::views::main_window::MailNavState::default(),
        pending_snooze_expired: Vec::new(),
        refresh_successes: 0,
        registry_has_server_error: false,
        registry_in_flight: Vec::new(),
        registry_last_synced_at: None,
        skills_left_pane_width: 0.0,
        skills_nav: Default::default(),
        sync: status_bar::SyncState::default(),
        toast: None,
        wallet_right_rail_width: 0.0,
      };

      apply_assets_loaded_to_cache(&mut state, &Ok(vec![]));

      assert!(state.cached_assets_state.is_none());
    }
  }

  mod save_sidebar_width_if_drag_end {
    use pretty_assertions::assert_eq;

    use super::*;

    fn make_state_with_assets_view(sidebar_width: f32) -> State {
      let mut assets_state = assets::new(vec![], vec![], sidebar_width, 220.0);
      assets_state.sidebar_width = sidebar_width;
      State {
        abyssals_filter_pane_width: 220.0,
        active_nav: Nav::Assets,
        active_view: ActiveView::Assets(assets_state),
        assets_sidebar_width: 0.0,
        cached_assets_state: None,
        cached_wallet_state: None,
        characters: Vec::new(),
        corporations: Vec::new(),
        esi_connected: false,
        eve_time: String::new(),
        feat_asset_tracking: false,
        feat_mail: false,
        feat_skill_monitoring: false,
        feat_wallet: false,
        hovered_nav: None,
        mail_folder_pane_width: 0.0,
        mail_message_list_width: 0.0,
        mail_nav: pod_ui::views::main_window::MailNavState::default(),
        pending_snooze_expired: Vec::new(),
        refresh_successes: 0,
        registry_has_server_error: false,
        registry_in_flight: Vec::new(),
        registry_last_synced_at: None,
        skills_left_pane_width: 0.0,
        skills_nav: Default::default(),
        sync: status_bar::SyncState::default(),
        toast: None,
        wallet_right_rail_width: 0.0,
      }
    }

    #[test]
    fn it_saves_sidebar_width_when_drag_ended() {
      let mut state = make_state_with_assets_view(300.0);

      save_sidebar_width_if_drag_end(&mut state, true);

      assert_eq!(state.assets_sidebar_width, 300.0);
    }

    #[test]
    fn it_does_nothing_when_not_drag_end() {
      let mut state = make_state_with_assets_view(300.0);

      save_sidebar_width_if_drag_end(&mut state, false);

      assert_eq!(state.assets_sidebar_width, 0.0);
    }
  }

  mod should_assets_refresh_values {
    use super::*;

    fn make_assets_state(active_tab: assets::Tab) -> assets::State {
      let mut s = assets::new(vec![], vec![], 232.0, 220.0);
      s.active_tab = active_tab;
      s
    }

    #[test]
    fn it_returns_true_when_tab_selected_is_values() {
      let s = make_assets_state(assets::Tab::Inventory);
      let result = should_assets_refresh_values(&assets::Message::TabSelected(assets::Tab::Values), &s);

      assert!(result);
    }

    #[test]
    fn it_returns_true_when_assets_loaded_and_active_tab_is_values() {
      let s = make_assets_state(assets::Tab::Values);
      let result = should_assets_refresh_values(&assets::Message::AssetsLoaded(Ok(vec![])), &s);

      assert!(result);
    }

    #[test]
    fn it_returns_false_when_assets_loaded_but_active_tab_is_not_values() {
      let s = make_assets_state(assets::Tab::Inventory);
      let result = should_assets_refresh_values(&assets::Message::AssetsLoaded(Ok(vec![])), &s);

      assert!(!result);
    }

    #[test]
    fn it_returns_false_for_other_messages() {
      let s = make_assets_state(assets::Tab::Values);
      let result = should_assets_refresh_values(&assets::Message::TabSelected(assets::Tab::Inventory), &s);

      assert!(!result);
    }
  }

  mod swap_active_view {
    use super::*;

    fn make_state() -> State {
      State {
        abyssals_filter_pane_width: 220.0,
        active_nav: Nav::Settings,
        active_view: ActiveView::Settings(settings::State::default()),
        assets_sidebar_width: 232.0,
        cached_assets_state: None,
        cached_wallet_state: None,
        characters: Vec::new(),
        corporations: Vec::new(),
        esi_connected: false,
        eve_time: String::new(),
        feat_asset_tracking: false,
        feat_mail: false,
        feat_skill_monitoring: false,
        feat_wallet: false,
        hovered_nav: None,
        mail_folder_pane_width: 0.0,
        mail_message_list_width: 0.0,
        mail_nav: pod_ui::views::main_window::MailNavState::default(),
        pending_snooze_expired: Vec::new(),
        refresh_successes: 0,
        registry_has_server_error: false,
        registry_in_flight: Vec::new(),
        registry_last_synced_at: None,
        skills_left_pane_width: 0.0,
        skills_nav: Default::default(),
        sync: status_bar::SyncState::default(),
        toast: None,
        wallet_right_rail_width: 0.0,
      }
    }

    #[test]
    fn it_saves_assets_state_to_cache_when_previous_view_was_assets() {
      let mut state = make_state();
      state.active_view = ActiveView::Assets(assets::new(vec![], vec![], 232.0, 220.0));

      swap_active_view(&mut state, ActiveView::Settings(settings::State::default()));

      assert!(state.cached_assets_state.is_some());
      assert!(matches!(state.active_view, ActiveView::Settings(_)));
    }

    #[test]
    fn it_does_not_touch_cache_when_previous_view_was_not_assets() {
      let mut state = make_state();

      swap_active_view(&mut state, ActiveView::Settings(settings::State::default()));

      assert!(state.cached_assets_state.is_none());
    }
  }
}
