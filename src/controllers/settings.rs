//! Settings controller: manages feature flag state and config persistence.

use pod_ui::views::settings::{Category, Message, State, TagSortMode, features_tab};

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
    tag_color_hex_draft: String::new(),
    tag_color_open: None,
    tag_draft: String::new(),
    tag_drag_over: None,
    tag_dragging: None,
    tag_editing: None,
    tag_new_name: String::new(),
    tag_search: String::new(),
    tag_sort_mode: TagSortMode::default(),
    tags: Vec::new(),
  };
  (state, iced::Task::none())
}

/// Processes a settings message, persists the config to disk, and
/// returns a task. The caller is responsible for updating `app.config`
/// by calling [`updated_config`] after any toggle or reset.
pub fn update(state: &mut State, msg: Message, services: &Services) -> iced::Task<Message> {
  match msg {
    Message::CategorySelected(cat) => {
      let needs_load = cat == Category::Tags && state.tags.is_empty();
      state.active_category = cat;
      if needs_load {
        load_tags_task(services)
      } else {
        iced::Task::none()
      }
    }
    Message::FeaturesTab(inner) => {
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
    Message::ResetDefaults => {
      tracing::info!("settings: reset to defaults");
      state.features.asset_tracking = true;
      state.features.clone_monitoring = true;
      state.features.combat_log = true;
      state.features.contacts = true;
      state.features.eve_notifications = true;
      state.features.location_tracking = true;
      state.features.mail = true;
      state.features.skill_monitoring = true;
      state.features.standings = true;
      state.features.wallet = true;
      iced::Task::none()
    }
    Message::TagColorClose => {
      state.tag_color_open = None;
      iced::Task::none()
    }
    Message::TagColorHexChanged(s) => {
      state.tag_color_hex_draft = s;
      iced::Task::none()
    }
    Message::TagColorHexCommit => {
      let Some(id) = state.tag_color_open else {
        return iced::Task::none();
      };
      let Some(normalized) = normalize_hex(&state.tag_color_hex_draft) else {
        return iced::Task::none();
      };
      let Some(db) = services.db.clone() else {
        return iced::Task::none();
      };
      let color = normalized.clone();
      state.tag_color_hex_draft = normalized;
      iced::Task::perform(
        async move {
          db.tags()
            .set_color(id, Some(&color))
            .await
            .map(|t| (t.id, t.name, t.color))
            .map_err(|e| e.to_string())
        },
        Message::TagColorSet,
      )
    }
    Message::TagColorOpen(id) => {
      state.tag_color_open = Some(id);
      state.tag_color_hex_draft = state
        .tags
        .iter()
        .find(|(tid, _, _)| *tid == id)
        .and_then(|(_, _, c)| c.clone())
        .unwrap_or_default();
      iced::Task::none()
    }
    Message::TagColorSet(result) => {
      match result {
        Ok((id, name, color)) => {
          if let Some(t) = state.tags.iter_mut().find(|(tid, _, _)| *tid == id) {
            *t = (id, name, color);
          }
        }
        Err(e) => tracing::error!("settings: set_color failed — {e}"),
      }
      state.tag_color_open = None;
      iced::Task::none()
    }
    Message::TagCreate => {
      let name = state.tag_new_name.trim().to_string();
      if name.is_empty() {
        return iced::Task::none();
      }
      let Some(db) = services.db.clone() else {
        return iced::Task::none();
      };
      state.tag_new_name.clear();
      iced::Task::perform(
        async move {
          db.tags()
            .create(&name)
            .await
            .map(|t| (t.id, t.name, t.color))
            .map_err(|e| e.to_string())
        },
        Message::TagCreated,
      )
    }
    Message::TagCreated(result) => {
      match result {
        Ok(tag) => state.tags.push(tag),
        Err(e) => tracing::error!("settings: tag create failed — {e}"),
      }
      iced::Task::none()
    }
    Message::TagDelete(id) => {
      let Some(db) = services.db.clone() else {
        return iced::Task::none();
      };
      iced::Task::perform(
        async move { db.tags().delete(id).await.map(|_| id).map_err(|e| e.to_string()) },
        Message::TagDeleted,
      )
    }
    Message::TagDeleted(result) => {
      match result {
        Ok(id) => {
          state.tags.retain(|(tid, _, _)| *tid != id);
          if state.tag_editing == Some(id) {
            state.tag_editing = None;
            state.tag_draft.clear();
          }
          if state.tag_color_open == Some(id) {
            state.tag_color_open = None;
          }
        }
        Err(e) => tracing::error!("settings: tag delete failed — {e}"),
      }
      iced::Task::none()
    }
    Message::TagDragEnd => {
      state.tag_dragging = None;
      state.tag_drag_over = None;
      iced::Task::none()
    }
    Message::TagDragStart(id) => {
      state.tag_dragging = Some(id);
      state.tag_drag_over = None;
      state.tag_color_open = None;
      state.tag_editing = None;
      iced::Task::none()
    }
    Message::TagDraftChanged(s) => {
      state.tag_draft = s;
      iced::Task::none()
    }
    Message::TagDrop => {
      let Some(drag_id) = state.tag_dragging.take() else {
        return iced::Task::none();
      };
      let target_id = state.tag_drag_over.take();
      if let Some(target) = target_id
        && drag_id != target
      {
        let from = state.tags.iter().position(|(id, _, _)| *id == drag_id);
        let to = state.tags.iter().position(|(id, _, _)| *id == target);
        if let (Some(from), Some(to)) = (from, to) {
          let item = state.tags.remove(from);
          let insert_at = if from < to { to - 1 } else { to };
          state.tags.insert(insert_at.min(state.tags.len()), item);
          return reorder_task(state, services);
        }
      }
      iced::Task::none()
    }
    Message::TagEditStart(id) => {
      let draft = state
        .tags
        .iter()
        .find(|(tid, _, _)| *tid == id)
        .map(|(_, n, _)| n.clone())
        .unwrap_or_default();
      state.tag_editing = Some(id);
      state.tag_draft = draft;
      state.tag_color_open = None;
      iced::Task::none()
    }
    Message::TagNewNameChanged(s) => {
      state.tag_new_name = s;
      iced::Task::none()
    }
    Message::TagRename => {
      let Some(id) = state.tag_editing else {
        return iced::Task::none();
      };
      let name = state.tag_draft.trim().to_string();
      if name.is_empty() {
        return iced::Task::none();
      }
      let Some(db) = services.db.clone() else {
        return iced::Task::none();
      };
      state.tag_editing = None;
      state.tag_draft.clear();
      iced::Task::perform(
        async move {
          db.tags()
            .rename(id, &name)
            .await
            .map(|t| (t.id, t.name, t.color))
            .map_err(|e| e.to_string())
        },
        Message::TagRenamed,
      )
    }
    Message::TagRenamed(result) => {
      match result {
        Ok((id, name, color)) => {
          if let Some(t) = state.tags.iter_mut().find(|(tid, _, _)| *tid == id) {
            *t = (id, name, color);
          }
        }
        Err(e) => tracing::error!("settings: tag rename failed — {e}"),
      }
      iced::Task::none()
    }
    Message::TagReordered(result) => {
      if let Err(e) = result {
        tracing::error!("settings: tag reorder failed — {e}");
      }
      iced::Task::none()
    }
    Message::TagSearchChanged(s) => {
      state.tag_search = s;
      iced::Task::none()
    }
    Message::TagSetColor(id, color) => {
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
        Message::TagColorSet,
      )
    }
    Message::TagSlotEntered(id) => {
      if state.tag_dragging.is_some() {
        state.tag_drag_over = Some(id);
      }
      iced::Task::none()
    }
    Message::TagSortModeChanged(mode) => {
      state.tag_sort_mode = mode;
      iced::Task::none()
    }
    Message::TagsLoaded(tags) => {
      state.tags = tags;
      iced::Task::none()
    }
  }
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
    Message::TagsLoaded,
  )
}

fn normalize_hex(raw: &str) -> Option<String> {
  let s = raw.trim().trim_start_matches('#');
  if s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit()) {
    Some(format!("#{}", s.to_uppercase()))
  } else if s.len() == 3 && s.chars().all(|c| c.is_ascii_hexdigit()) {
    let expanded: String = s.chars().flat_map(|c| [c, c]).collect();
    Some(format!("#{}", expanded.to_uppercase()))
  } else {
    None
  }
}

fn reorder_task(state: &State, services: &Services) -> iced::Task<Message> {
  let Some(db) = services.db.clone() else {
    return iced::Task::none();
  };
  let ids: Vec<i32> = state.tags.iter().map(|(id, _, _)| *id).collect();
  iced::Task::perform(
    async move { db.tags().reorder(&ids).await.map_err(|e| e.to_string()) },
    Message::TagReordered,
  )
}

fn toggle_feature(features: &mut features_tab::State, feature: &features_tab::Feature) {
  match feature {
    features_tab::Feature::AssetTracking => features.asset_tracking = !features.asset_tracking,
    features_tab::Feature::CloneMonitoring => features.clone_monitoring = !features.clone_monitoring,
    features_tab::Feature::CombatLog => features.combat_log = !features.combat_log,
    features_tab::Feature::Contacts => features.contacts = !features.contacts,
    features_tab::Feature::EveNotifications => features.eve_notifications = !features.eve_notifications,
    features_tab::Feature::LocationTracking => features.location_tracking = !features.location_tracking,
    features_tab::Feature::Mail => features.mail = !features.mail,
    features_tab::Feature::SkillMonitoring => features.skill_monitoring = !features.skill_monitoring,
    features_tab::Feature::Standings => features.standings = !features.standings,
    features_tab::Feature::Wallet => features.wallet = !features.wallet,
  }
}
