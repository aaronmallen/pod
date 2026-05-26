//! Settings controller: manages feature flag state and config persistence.

use std::path::PathBuf;

use pod_ui::{
  components::color_picker::normalize_hex,
  views::settings::{Category, Message, State, features_tab, storage_tab, tags_tab},
};

use crate::services::Services;

/// Creates initial settings state from the current config.
pub fn new(config: &crate::config::Settings) -> (State, iced::Task<Message>) {
  let features = config.features();
  let storage_cfg = config.storage();
  let state = State {
    active_category: Category::default(),
    features: features_tab::State {
      asset_tracking: *features.asset_tracking(),
      clone_monitoring: *features.clone_monitoring(),
      combat_log: *features.combat_log(),
      contacts: *features.contacts(),
      eve_notifications: *features.eve_notifications(),
      location_tracking: *features.location_tracking(),
      mail: *features.mail(),
      search_query: String::new(),
      skill_monitoring: *features.skill_monitoring(),
      standings: *features.standings(),
      wallet: *features.wallet(),
    },
    storage: storage_tab::State::from_paths(
      storage_cfg.db_dir().as_ref(),
      storage_cfg.cache_dir().as_ref(),
      storage_cfg.log_dir().as_ref(),
      default_storage_path(&storage_tab::PathId::DbDir)
        .map(|p| p.display().to_string())
        .unwrap_or_default(),
      default_storage_path(&storage_tab::PathId::CacheDir)
        .map(|p| p.display().to_string())
        .unwrap_or_default(),
      default_storage_path(&storage_tab::PathId::LogDir)
        .map(|p| p.display().to_string())
        .unwrap_or_default(),
    ),
    tags: tags_tab::State::default(),
  };
  (state, iced::Task::none())
}

/// Result of a settings message update that may include a new config.
///
/// `(task, restart_required_msg, new_config)`
pub type UpdateResult = (iced::Task<Message>, Option<String>, Option<crate::config::Settings>);

/// Processes a settings message, persists the config to disk, and
/// returns a task. The caller is responsible for updating `app.config`
/// when `new_config` is `Some`.
///
/// Returns `(task, restart_required_msg, new_config)`.
pub fn update(state: &mut State, msg: Message, services: &Services) -> UpdateResult {
  match msg {
    Message::CategorySelected(cat) => (handle_category_selected(state, cat, services), None, None),
    Message::FeaturesTab(inner) => (handle_features_tab(state, inner), None, None),
    Message::ResetDefaults => {
      reset_features_defaults(&mut state.features);
      (iced::Task::none(), None, None)
    }
    Message::StorageBrowse(id) => (handle_storage_browse(id), None, None),
    Message::StorageConfirmCancel(id) => {
      handle_storage_confirm_cancel(state, id);
      (iced::Task::none(), None, None)
    }
    Message::StorageConfirmMove(id) => handle_storage_confirm_move(state, id, services),
    Message::StorageConfirmSkip(id) => handle_storage_confirm_skip(state, id, services),
    Message::StoragePathCommit(id) => handle_storage_path_commit(state, id, services),
    Message::StoragePathSelected(id, Some(path)) => {
      handle_storage_path_selected(state, id, path);
      (iced::Task::none(), None, None)
    }
    Message::StoragePathSelected(_, None) => (iced::Task::none(), None, None),
    Message::StorageResetPath(id) => handle_storage_reset_path(state, id, services),
    Message::StorageTab(inner) => handle_storage_tab_inner(state, inner, services),
    Message::TagsTab(inner) => (update_tags(state, inner, services), None, None),
  }
}

fn handle_category_selected(state: &mut State, cat: Category, services: &Services) -> iced::Task<Message> {
  let needs_load = cat == Category::Tags && state.tags.tags.is_empty();
  state.active_category = cat;
  if needs_load {
    load_tags_task(services)
  } else {
    iced::Task::none()
  }
}

fn storage_browse_label(id: &storage_tab::PathId) -> &'static str {
  match id {
    storage_tab::PathId::CacheDir => "Choose Cache Directory",
    storage_tab::PathId::DbDir => "Choose Database Directory",
    storage_tab::PathId::LogDir => "Choose Log Directory",
  }
}

fn handle_storage_browse(id: storage_tab::PathId) -> iced::Task<Message> {
  let label = storage_browse_label(&id).to_string();
  iced::Task::perform(
    async move {
      rfd::AsyncFileDialog::new()
        .set_title(label)
        .pick_folder()
        .await
        .map(|f| f.path().to_path_buf())
    },
    move |path| Message::StoragePathSelected(id, path),
  )
}

fn current_storage_path(id: &storage_tab::PathId, config: &crate::config::Settings) -> Option<PathBuf> {
  let storage = config.storage();
  match id {
    storage_tab::PathId::CacheDir => storage.cache_dir().clone(),
    storage_tab::PathId::DbDir => storage.db_dir().clone(),
    storage_tab::PathId::LogDir => storage.log_dir().clone(),
  }
}

fn default_storage_path(id: &storage_tab::PathId) -> Option<PathBuf> {
  match id {
    storage_tab::PathId::CacheDir => dir_spec::cache_home().map(|p| p.join("pod")),
    storage_tab::PathId::DbDir => dir_spec::data_home().map(|p| p.join("pod")),
    storage_tab::PathId::LogDir => dir_spec::state_home().map(|p| p.join("pod").join("logs")),
  }
}

fn effective_current_path(id: &storage_tab::PathId, config: &crate::config::Settings) -> Option<PathBuf> {
  current_storage_path(id, config).or_else(|| default_storage_path(id))
}

fn has_existing_data(id: &storage_tab::PathId, current: &std::path::Path) -> bool {
  match id {
    storage_tab::PathId::DbDir => current.join("pod.db").exists(),
    storage_tab::PathId::CacheDir | storage_tab::PathId::LogDir => std::fs::read_dir(current)
      .map(|mut d| d.next().is_some())
      .unwrap_or(false),
  }
}

fn handle_storage_path_selected(state: &mut State, id: storage_tab::PathId, path: PathBuf) {
  let row = state.storage.row_mut(&id);
  row.draft = path.display().to_string();
}

fn handle_storage_path_commit(state: &mut State, id: storage_tab::PathId, services: &Services) -> UpdateResult {
  let new_path_str = state.storage.row(&id).draft.clone();
  if new_path_str.is_empty() {
    return (iced::Task::none(), None, None);
  }
  let new_path = PathBuf::from(&new_path_str);
  let Some(current) = effective_current_path(&id, &services.config) else {
    return save_storage_path(state, id, Some(new_path), services);
  };
  if current == new_path {
    return (iced::Task::none(), None, None);
  }
  if has_existing_data(&id, &current) {
    let row = state.storage.row_mut(&id);
    row.previous = current.display().to_string();
    row.confirm_move = true;
    return (iced::Task::none(), None, None);
  }
  save_storage_path(state, id, Some(new_path), services)
}

fn handle_storage_confirm_cancel(state: &mut State, id: storage_tab::PathId) {
  let row = state.storage.row_mut(&id);
  row.draft = row.previous.clone();
  row.confirm_move = false;
}

fn handle_storage_confirm_skip(state: &mut State, id: storage_tab::PathId, services: &Services) -> UpdateResult {
  let row = state.storage.row_mut(&id);
  row.confirm_move = false;
  let new_path = PathBuf::from(row.draft.clone());
  save_storage_path(state, id, Some(new_path), services)
}

fn handle_storage_confirm_move(state: &mut State, id: storage_tab::PathId, _services: &Services) -> UpdateResult {
  let row = state.storage.row_mut(&id);
  row.confirm_move = false;
  let old_path = PathBuf::from(row.previous.clone());
  let new_path = PathBuf::from(row.draft.clone());
  let id_for_async = id.clone();
  let id_for_map = id;
  let task = iced::Task::perform(
    async move { move_storage_data(&id_for_async, &old_path, &new_path).await },
    move |result| match result {
      Ok(()) => Message::StoragePathCommit(id_for_map.clone()),
      Err(e) => {
        tracing::error!("storage: move failed — {e}");
        Message::StorageConfirmSkip(id_for_map)
      }
    },
  );
  (task, None, None)
}

async fn move_storage_data(
  id: &storage_tab::PathId,
  old: &std::path::Path,
  new: &std::path::Path,
) -> Result<(), String> {
  if let Err(e) = tokio::fs::create_dir_all(new).await {
    return Err(format!("create_dir_all failed: {e}"));
  }
  match id {
    storage_tab::PathId::DbDir => move_db_files(old, new).await,
    storage_tab::PathId::CacheDir | storage_tab::PathId::LogDir => move_dir_contents(old, new).await,
  }
}

async fn move_db_files(old: &std::path::Path, new: &std::path::Path) -> Result<(), String> {
  let files = ["pod.db", "pod.db-shm", "pod.db-wal"];
  for name in files {
    let src = old.join(name);
    if !src.exists() {
      continue;
    }
    let dst = new.join(name);
    move_single_file(&src, &dst).await?;
  }
  Ok(())
}

async fn move_single_file(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
  match tokio::fs::rename(src, dst).await {
    Ok(()) => Ok(()),
    Err(_) => copy_then_delete(src, dst).await,
  }
}

async fn copy_then_delete(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
  tokio::fs::copy(src, dst)
    .await
    .map_err(|e| format!("copy failed: {e}"))?;
  tokio::fs::remove_file(src)
    .await
    .map_err(|e| format!("remove after copy failed: {e}"))?;
  Ok(())
}

async fn move_dir_contents(old: &std::path::Path, new: &std::path::Path) -> Result<(), String> {
  let mut rd = match tokio::fs::read_dir(old).await {
    Ok(r) => r,
    Err(_) => return Ok(()),
  };
  while let Ok(Some(entry)) = rd.next_entry().await {
    let src = entry.path();
    let Some(name) = src.file_name() else { continue };
    let dst = new.join(name);
    move_single_file(&src, &dst).await?;
  }
  Ok(())
}

fn save_storage_path(
  state: &mut State,
  id: storage_tab::PathId,
  path: Option<PathBuf>,
  services: &Services,
) -> UpdateResult {
  let row = state.storage.row_mut(&id);
  let display = path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
  row.draft = display.clone();
  row.previous = display;
  row.confirm_move = false;
  let new_config = updated_storage_config(&id, path, &services.config);
  (
    iced::Task::none(),
    Some("Path changes saved".to_string()),
    Some(new_config),
  )
}

fn handle_storage_reset_path(state: &mut State, id: storage_tab::PathId, services: &Services) -> UpdateResult {
  let row = state.storage.row_mut(&id);
  row.draft = String::new();
  row.previous = String::new();
  row.confirm_move = false;
  let new_config = updated_storage_config(&id, None, &services.config);
  (
    iced::Task::none(),
    Some("Path changes saved".to_string()),
    Some(new_config),
  )
}

fn handle_storage_tab_inner(state: &mut State, inner: storage_tab::Message, services: &Services) -> UpdateResult {
  match inner {
    storage_tab::Message::Browse(id) => (handle_storage_browse(id), None, None),
    storage_tab::Message::Commit(id) => handle_storage_path_commit(state, id, services),
    storage_tab::Message::ConfirmCancel(id) => {
      handle_storage_confirm_cancel(state, id);
      (iced::Task::none(), None, None)
    }
    storage_tab::Message::ConfirmMove(id) => handle_storage_confirm_move(state, id, services),
    storage_tab::Message::ConfirmSkip(id) => handle_storage_confirm_skip(state, id, services),
    storage_tab::Message::Edited(id, text) => {
      state.storage.row_mut(&id).draft = text;
      (iced::Task::none(), None, None)
    }
    storage_tab::Message::PathSelected(id, path) => {
      handle_storage_path_selected(state, id.clone(), path);
      (iced::Task::none(), None, None)
    }
    storage_tab::Message::ResetPath(id) => handle_storage_reset_path(state, id, services),
  }
}

fn handle_features_tab(state: &mut State, inner: features_tab::Message) -> iced::Task<Message> {
  match inner {
    features_tab::Message::SearchChanged(q) => {
      state.features.search_query = q;
    }
    features_tab::Message::ToggleFeature(feature) => {
      tracing::info!("settings: feature toggled — {feature:?}");
      toggle_feature(&mut state.features, &feature);
    }
  }
  iced::Task::none()
}

fn reset_features_defaults(features: &mut features_tab::State) {
  tracing::info!("settings: reset to defaults");
  features.asset_tracking = true;
  features.clone_monitoring = true;
  features.combat_log = true;
  features.contacts = true;
  features.eve_notifications = true;
  features.location_tracking = true;
  features.mail = true;
  features.skill_monitoring = true;
  features.standings = true;
  features.wallet = true;
}

/// Builds a new [`crate::config::Settings`] from the current state,
/// persists it to disk, and returns it so the caller can update
/// `app.config`.
pub fn updated_config(state: &State, current: &crate::config::Settings) -> crate::config::Settings {
  let features = crate::config::features::Settings::from_flags(
    state.features.asset_tracking,
    state.features.clone_monitoring,
    state.features.combat_log,
    state.features.contacts,
    state.features.eve_notifications,
    state.features.location_tracking,
    state.features.mail,
    state.features.skill_monitoring,
    state.features.standings,
    state.features.wallet,
  );
  let mut config = current.clone();
  config.set_features(features);
  crate::config::save(&config);
  config
}

/// Builds an updated config with the given storage path override applied,
/// persists it to disk, and returns it so the caller can update `app.config`.
pub fn updated_storage_config(
  id: &storage_tab::PathId,
  path: Option<PathBuf>,
  current: &crate::config::Settings,
) -> crate::config::Settings {
  let mut storage = current.storage().clone();
  match id {
    storage_tab::PathId::CacheDir => storage.set_cache_dir(path),
    storage_tab::PathId::DbDir => storage.set_db_dir(path),
    storage_tab::PathId::LogDir => storage.set_log_dir(path),
  }
  let mut config = current.clone();
  config.set_storage(storage);
  crate::config::save(&config);
  config
}

fn load_tags_task(services: &Services) -> iced::Task<Message> {
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  iced::Task::perform(
    async move {
      db.tags()
        .find_all()
        .await
        .map(|tags| tags.into_iter().map(|t| (t.id, t.name, t.color)).collect::<Vec<_>>())
        .unwrap_or_default()
    },
    |tags| Message::TagsTab(tags_tab::Message::Loaded(tags)),
  )
}

fn reorder_task(tags: &tags_tab::State, services: &Services) -> iced::Task<Message> {
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  let ids: Vec<i32> = tags.tags.iter().map(|(id, _, _)| *id).collect();
  iced::Task::perform(
    async move { db.tags().reorder(&ids).await.map_err(|e| e.to_string()) },
    |result| Message::TagsTab(tags_tab::Message::Reordered(result)),
  )
}

fn toggle_feature(features: &mut features_tab::State, feature: &features_tab::Feature) {
  if !toggle_feature_tracking(features, feature) {
    toggle_feature_social(features, feature);
  }
}

fn toggle_feature_tracking(features: &mut features_tab::State, feature: &features_tab::Feature) -> bool {
  match feature {
    features_tab::Feature::AssetTracking | features_tab::Feature::CloneMonitoring => {
      toggle_tracking_movement(features, feature);
    }
    features_tab::Feature::CombatLog
    | features_tab::Feature::LocationTracking
    | features_tab::Feature::SkillMonitoring => {
      toggle_tracking_activity(features, feature);
    }
    _ => return false,
  }
  true
}

fn toggle_tracking_movement(features: &mut features_tab::State, feature: &features_tab::Feature) {
  match feature {
    features_tab::Feature::AssetTracking => features.asset_tracking = !features.asset_tracking,
    features_tab::Feature::CloneMonitoring => features.clone_monitoring = !features.clone_monitoring,
    _ => {}
  }
}

fn toggle_tracking_activity(features: &mut features_tab::State, feature: &features_tab::Feature) {
  match feature {
    features_tab::Feature::CombatLog => features.combat_log = !features.combat_log,
    features_tab::Feature::LocationTracking => features.location_tracking = !features.location_tracking,
    features_tab::Feature::SkillMonitoring => features.skill_monitoring = !features.skill_monitoring,
    _ => {}
  }
}

fn toggle_feature_social(features: &mut features_tab::State, feature: &features_tab::Feature) {
  match feature {
    features_tab::Feature::Contacts | features_tab::Feature::EveNotifications | features_tab::Feature::Mail => {
      toggle_social_communications(features, feature);
    }
    features_tab::Feature::Standings | features_tab::Feature::Wallet => {
      toggle_social_economy(features, feature);
    }
    _ => {}
  }
}

fn toggle_social_communications(features: &mut features_tab::State, feature: &features_tab::Feature) {
  match feature {
    features_tab::Feature::Contacts => features.contacts = !features.contacts,
    features_tab::Feature::EveNotifications => features.eve_notifications = !features.eve_notifications,
    features_tab::Feature::Mail => features.mail = !features.mail,
    _ => {}
  }
}

fn toggle_social_economy(features: &mut features_tab::State, feature: &features_tab::Feature) {
  match feature {
    features_tab::Feature::Standings => features.standings = !features.standings,
    features_tab::Feature::Wallet => features.wallet = !features.wallet,
    _ => {}
  }
}

fn update_tags(state: &mut State, msg: tags_tab::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    tags_tab::Message::ColorClose
    | tags_tab::Message::ColorOpen(_)
    | tags_tab::Message::ColorSet(_)
    | tags_tab::Message::SetColor(_, _) => update_tags_color(state, msg, services),
    tags_tab::Message::Create
    | tags_tab::Message::Created(_)
    | tags_tab::Message::Delete(_)
    | tags_tab::Message::Deleted(_) => update_tags_crud(state, msg, services),
    tags_tab::Message::DragEnd
    | tags_tab::Message::DragStart(_)
    | tags_tab::Message::Drop
    | tags_tab::Message::SlotEntered(_) => update_tags_drag(state, msg, services),
    tags_tab::Message::HexChanged(_) | tags_tab::Message::HexSubmit => update_tags_hex(state, msg, services),
    _ => update_tags_interaction(state, msg, services),
  }
}

fn update_tags_crud(state: &mut State, msg: tags_tab::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    tags_tab::Message::Create | tags_tab::Message::Created(_) => update_tags_create(state, msg, services),
    tags_tab::Message::Delete(_) | tags_tab::Message::Deleted(_) => update_tags_delete(state, msg, services),
    _ => iced::Task::none(),
  }
}

fn update_tags_hex(state: &mut State, msg: tags_tab::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    tags_tab::Message::HexChanged(s) => {
      state.tags.hex_picker.hex_draft = s;
      state.tags.hex_picker.hex_error = false;
      iced::Task::none()
    }
    tags_tab::Message::HexSubmit => {
      let Some(id) = state.tags.color_open else {
        return iced::Task::none();
      };
      match normalize_hex(&state.tags.hex_picker.hex_draft) {
        Some(hex) => {
          state.tags.hex_picker.set_from_selection(&hex);
          dispatch_set_color(id, Some(hex), services)
        }
        None => {
          state.tags.hex_picker.hex_error = true;
          iced::Task::none()
        }
      }
    }
    _ => iced::Task::none(),
  }
}

fn update_tags_interaction(state: &mut State, msg: tags_tab::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    tags_tab::Message::DraftChanged(_)
    | tags_tab::Message::EditStart(_)
    | tags_tab::Message::Rename
    | tags_tab::Message::Renamed(_) => update_tags_edit(state, msg, services),
    _ => update_tags_misc(state, msg),
  }
}

fn apply_color_set(state: &mut State, result: Result<(i32, String, Option<String>), String>) {
  match result {
    Ok((id, name, color)) => {
      if let Some(t) = state.tags.tags.iter_mut().find(|(tid, _, _)| *tid == id) {
        *t = (id, name, color);
      }
    }
    Err(e) => tracing::error!("settings: set_color failed — {e}"),
  }
  state.tags.color_open = None;
}

fn dispatch_set_color(id: i32, color: Option<String>, services: &Services) -> iced::Task<Message> {
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  iced::Task::perform(
    async move {
      db.tags()
        .set_color(id, color.as_deref())
        .await
        .map(|t| (t.id, t.name, t.color))
        .map_err(|e| e.to_string())
    },
    |result| Message::TagsTab(tags_tab::Message::ColorSet(result)),
  )
}

fn update_tags_color_state(state: &mut State, msg: tags_tab::Message) -> iced::Task<Message> {
  match msg {
    tags_tab::Message::ColorClose => state.tags.color_open = None,
    tags_tab::Message::ColorOpen(id) => {
      let current_color = state
        .tags
        .tags
        .iter()
        .find(|(tid, _, _)| *tid == id)
        .and_then(|(_, _, c)| c.as_deref())
        .unwrap_or("");
      state.tags.hex_picker.open(current_color);
      state.tags.color_open = Some(id);
      state.tags.editing = None;
    }
    _ => {}
  }
  iced::Task::none()
}

fn update_tags_color(state: &mut State, msg: tags_tab::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    tags_tab::Message::ColorSet(result) => {
      apply_color_set(state, result);
      iced::Task::none()
    }
    tags_tab::Message::SetColor(id, color) => {
      match &color {
        Some(hex) => state.tags.hex_picker.set_from_selection(hex),
        None => state.tags.hex_picker.clear(),
      }
      dispatch_set_color(id, color, services)
    }
    msg => update_tags_color_state(state, msg),
  }
}

fn dispatch_create_tag(state: &mut State, services: &Services) -> iced::Task<Message> {
  let name = state.tags.new_name.trim().to_string();
  if name.is_empty() {
    return iced::Task::none();
  }
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  state.tags.new_name.clear();
  iced::Task::perform(
    async move {
      db.tags()
        .create(&name)
        .await
        .map(|t| (t.id, t.name, t.color))
        .map_err(|e| e.to_string())
    },
    |result| Message::TagsTab(tags_tab::Message::Created(result)),
  )
}

fn apply_tag_created(state: &mut State, result: Result<(i32, String, Option<String>), String>) {
  match result {
    Ok(tag) => state.tags.tags.push(tag),
    Err(e) => tracing::error!("settings: tag create failed — {e}"),
  }
}

fn update_tags_create(state: &mut State, msg: tags_tab::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    tags_tab::Message::Create => dispatch_create_tag(state, services),
    tags_tab::Message::Created(result) => {
      apply_tag_created(state, result);
      iced::Task::none()
    }
    _ => iced::Task::none(),
  }
}

fn dispatch_delete(id: i32, services: &Services) -> iced::Task<Message> {
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  iced::Task::perform(
    async move { db.tags().delete(id).await.map(|_| id).map_err(|e| e.to_string()) },
    |result| Message::TagsTab(tags_tab::Message::Deleted(result)),
  )
}

fn clear_deleted_tag_ui(state: &mut State, id: i32) {
  if state.tags.editing == Some(id) {
    state.tags.editing = None;
    state.tags.draft.clear();
  }
  if state.tags.color_open == Some(id) {
    state.tags.color_open = None;
  }
}

fn apply_tag_deleted(state: &mut State, result: Result<i32, String>) {
  match result {
    Ok(id) => {
      state.tags.tags.retain(|(tid, _, _)| *tid != id);
      clear_deleted_tag_ui(state, id);
    }
    Err(e) => tracing::error!("settings: tag delete failed — {e}"),
  }
}

fn update_tags_delete(state: &mut State, msg: tags_tab::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    tags_tab::Message::Delete(id) => dispatch_delete(id, services),
    tags_tab::Message::Deleted(result) => {
      apply_tag_deleted(state, result);
      iced::Task::none()
    }
    _ => iced::Task::none(),
  }
}

fn update_tags_drag(state: &mut State, msg: tags_tab::Message, services: &Services) -> iced::Task<Message> {
  apply_drag_state(state, &msg);
  match msg {
    tags_tab::Message::Drop => apply_tag_drop(state, services),
    _ => iced::Task::none(),
  }
}

fn apply_drag_slot_entered(state: &mut State, id: i32) {
  if state.tags.dragging.is_some() {
    state.tags.drag_over = Some(id);
  }
}

fn apply_drag_state(state: &mut State, msg: &tags_tab::Message) {
  match msg {
    tags_tab::Message::DragEnd => {
      state.tags.dragging = None;
      state.tags.drag_over = None;
    }
    tags_tab::Message::DragStart(id) => {
      state.tags.dragging = Some(*id);
      state.tags.drag_over = None;
      state.tags.color_open = None;
      state.tags.editing = None;
    }
    tags_tab::Message::SlotEntered(id) => apply_drag_slot_entered(state, *id),
    _ => {}
  }
}

fn tag_drop_positions(tags: &[(i32, String, Option<String>)], drag_id: i32, target: i32) -> Option<(usize, usize)> {
  let from = tags.iter().position(|(id, _, _)| *id == drag_id)?;
  let to = tags.iter().position(|(id, _, _)| *id == target)?;
  Some((from, to))
}

fn reorder_tags(tags: &mut Vec<(i32, String, Option<String>)>, from: usize, to: usize) {
  let item = tags.remove(from);
  let insert_at = if from < to { to - 1 } else { to };
  tags.insert(insert_at.min(tags.len()), item);
}

fn apply_tag_drop(state: &mut State, services: &Services) -> iced::Task<Message> {
  let Some(drag_id) = state.tags.dragging.take() else {
    return iced::Task::none();
  };
  let Some(target) = state.tags.drag_over.take() else {
    return iced::Task::none();
  };
  if drag_id == target {
    return iced::Task::none();
  }
  if let Some((from, to)) = tag_drop_positions(&state.tags.tags, drag_id, target) {
    reorder_tags(&mut state.tags.tags, from, to);
    return reorder_task(&state.tags, services);
  }
  iced::Task::none()
}

fn apply_edit_start(state: &mut State, id: i32) {
  let draft = state
    .tags
    .tags
    .iter()
    .find(|(tid, _, _)| *tid == id)
    .map(|(_, n, _)| n.clone())
    .unwrap_or_default();
  state.tags.editing = Some(id);
  state.tags.draft = draft;
  state.tags.color_open = None;
}

fn apply_tag_renamed(state: &mut State, result: Result<(i32, String, Option<String>), String>) {
  match result {
    Ok((id, name, color)) => {
      if let Some(t) = state.tags.tags.iter_mut().find(|(tid, _, _)| *tid == id) {
        *t = (id, name, color);
      }
    }
    Err(e) => tracing::error!("settings: tag rename failed — {e}"),
  }
}

fn update_tags_edit_state(state: &mut State, msg: tags_tab::Message) -> iced::Task<Message> {
  match msg {
    tags_tab::Message::DraftChanged(s) => state.tags.draft = s,
    tags_tab::Message::EditStart(id) => apply_edit_start(state, id),
    _ => {}
  }
  iced::Task::none()
}

fn update_tags_edit(state: &mut State, msg: tags_tab::Message, services: &Services) -> iced::Task<Message> {
  match msg {
    tags_tab::Message::Rename => submit_tag_rename(state, services),
    tags_tab::Message::Renamed(result) => {
      apply_tag_renamed(state, result);
      iced::Task::none()
    }
    msg => update_tags_edit_state(state, msg),
  }
}

fn submit_tag_rename(state: &mut State, services: &Services) -> iced::Task<Message> {
  let Some(id) = state.tags.editing else {
    return iced::Task::none();
  };
  let name = state.tags.draft.trim().to_string();
  if name.is_empty() {
    return iced::Task::none();
  }
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  state.tags.editing = None;
  state.tags.draft.clear();
  iced::Task::perform(
    async move {
      db.tags()
        .rename(id, &name)
        .await
        .map(|t| (t.id, t.name, t.color))
        .map_err(|e| e.to_string())
    },
    |result| Message::TagsTab(tags_tab::Message::Renamed(result)),
  )
}

fn update_tags_misc_str_state(state: &mut State, msg: &tags_tab::Message) {
  match msg {
    tags_tab::Message::NewNameChanged(s) => state.tags.new_name = s.clone(),
    tags_tab::Message::SearchChanged(s) => state.tags.search = s.clone(),
    _ => {}
  }
}

fn update_tags_misc_state(state: &mut State, msg: &tags_tab::Message) {
  match msg {
    tags_tab::Message::Loaded(tags) => state.tags.tags = tags.clone(),
    tags_tab::Message::SortModeChanged(mode) => state.tags.sort_mode = mode.clone(),
    msg => update_tags_misc_str_state(state, msg),
  }
}

fn update_tags_misc(state: &mut State, msg: tags_tab::Message) -> iced::Task<Message> {
  if let tags_tab::Message::Reordered(Err(e)) = &msg {
    tracing::error!("settings: tag reorder failed — {e}");
  } else {
    update_tags_misc_state(state, &msg);
  }
  iced::Task::none()
}
