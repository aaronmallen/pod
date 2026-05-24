//! Settings controller: manages feature flag state and config persistence.

use pod_ui::views::settings::{Category, Feature, Message, State};

use crate::services::Services;

/// Creates initial settings state from the current feature config.
pub fn new(features: &crate::config::features::Settings) -> (State, iced::Task<Message>) {
  let state = State {
    active_category: Category::default(),
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
    tag_color_open: None,
    tag_draft: String::new(),
    tag_editing: None,
    tag_new_name: String::new(),
    tags: Vec::new(),
    wallet: *features.wallet(),
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
    Message::ResetDefaults => {
      tracing::info!("settings: reset to defaults");
      state.asset_tracking = true;
      state.clone_monitoring = true;
      state.combat_log = true;
      state.contacts = true;
      state.eve_notifications = true;
      state.location_tracking = true;
      state.mail = true;
      state.skill_monitoring = true;
      state.standings = true;
      state.wallet = true;
      iced::Task::none()
    }
    Message::SearchChanged(q) => {
      state.search_query = q;
      iced::Task::none()
    }
    Message::TagColorClose => {
      state.tag_color_open = None;
      iced::Task::none()
    }
    Message::TagColorOpen(id) => {
      state.tag_color_open = Some(id);
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
    Message::TagDraftChanged(s) => {
      state.tag_draft = s;
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
    Message::TagMoveDown(id) => {
      let Some(pos) = state.tags.iter().position(|(tid, _, _)| *tid == id) else {
        return iced::Task::none();
      };
      if pos + 1 >= state.tags.len() {
        return iced::Task::none();
      }
      state.tags.swap(pos, pos + 1);
      reorder_task(state, services)
    }
    Message::TagMoveUp(id) => {
      let Some(pos) = state.tags.iter().position(|(tid, _, _)| *tid == id) else {
        return iced::Task::none();
      };
      if pos == 0 {
        return iced::Task::none();
      }
      state.tags.swap(pos, pos - 1);
      reorder_task(state, services)
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
    Message::TagsLoaded(tags) => {
      state.tags = tags;
      iced::Task::none()
    }
    Message::ToggleFeature(feature) => {
      tracing::info!("settings: feature toggled — {feature:?}");
      toggle_feature(state, &feature);
      iced::Task::none()
    }
  }
}

/// Builds a new [`crate::config::Settings`] from the current state,
/// persists it to disk, and returns it so the caller can update
/// `app.config`.
pub fn updated_config(state: &State, current: &crate::config::Settings) -> crate::config::Settings {
  let features = crate::config::features::Settings::from_flags(
    state.asset_tracking,
    state.clone_monitoring,
    state.combat_log,
    state.contacts,
    state.eve_notifications,
    state.location_tracking,
    state.mail,
    state.skill_monitoring,
    state.standings,
    state.wallet,
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

fn toggle_feature(state: &mut State, feature: &Feature) {
  match feature {
    Feature::AssetTracking => state.asset_tracking = !state.asset_tracking,
    Feature::CloneMonitoring => state.clone_monitoring = !state.clone_monitoring,
    Feature::CombatLog => state.combat_log = !state.combat_log,
    Feature::Contacts => state.contacts = !state.contacts,
    Feature::EveNotifications => state.eve_notifications = !state.eve_notifications,
    feature => toggle_feature_b(state, feature),
  }
}

fn toggle_feature_b(state: &mut State, feature: &Feature) {
  match feature {
    Feature::LocationTracking => state.location_tracking = !state.location_tracking,
    Feature::Mail => state.mail = !state.mail,
    Feature::SkillMonitoring => state.skill_monitoring = !state.skill_monitoring,
    Feature::Standings => state.standings = !state.standings,
    Feature::Wallet => state.wallet = !state.wallet,
    _ => {}
  }
}
