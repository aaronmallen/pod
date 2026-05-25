//! Settings controller: manages feature flag state and config persistence.

use pod_ui::views::settings::{Category, Message, State, features_tab, tags_tab};

use crate::services::Services;

/// Creates initial settings state from the current feature config.
pub fn new(features: &crate::config::features::Settings) -> (State, iced::Task<Message>) {
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
    tags: tags_tab::State::default(),
  };
  (state, iced::Task::none())
}

/// Processes a settings message, persists the config to disk, and
/// returns a task. The caller is responsible for updating `app.config`
/// by calling [`updated_config`] after any toggle or reset.
pub fn update(state: &mut State, msg: Message, services: &Services) -> iced::Task<Message> {
  match msg {
    Message::CategorySelected(cat) => handle_category_selected(state, cat, services),
    Message::FeaturesTab(inner) => handle_features_tab(state, inner),
    Message::ResetDefaults => {
      reset_features_defaults(&mut state.features);
      iced::Task::none()
    }
    Message::TagsTab(inner) => update_tags(state, inner, services),
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
    tags_tab::Message::SetColor(id, color) => dispatch_set_color(id, color, services),
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
