//! Settings controller: manages feature flag state and config persistence.

use pod_ui::views::settings::{Feature, Message, State};

use crate::services::Services;

/// Creates initial settings state from the current feature config.
pub fn new(features: &crate::config::features::Settings) -> (State, iced::Task<Message>) {
  let state = State {
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
  };
  (state, iced::Task::none())
}

/// Processes a settings message, persists the config to disk, and
/// returns a task. The caller is responsible for updating `app.config`
/// by calling [`updated_config`] after any toggle or reset.
pub fn update(state: &mut State, msg: Message, _services: &Services) -> iced::Task<Message> {
  match msg {
    Message::ResetDefaults => {
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
    Message::ToggleFeature(feature) => {
      toggle_feature(state, &feature);
      iced::Task::none()
    }
  }
}

fn toggle_feature(state: &mut State, feature: &Feature) {
  match feature {
    Feature::AssetTracking => state.asset_tracking = !state.asset_tracking,
    Feature::CloneMonitoring => state.clone_monitoring = !state.clone_monitoring,
    Feature::CombatLog => state.combat_log = !state.combat_log,
    Feature::Contacts => state.contacts = !state.contacts,
    Feature::EveNotifications => state.eve_notifications = !state.eve_notifications,
    Feature::LocationTracking => state.location_tracking = !state.location_tracking,
    Feature::Mail => state.mail = !state.mail,
    Feature::SkillMonitoring => state.skill_monitoring = !state.skill_monitoring,
    Feature::Standings => state.standings = !state.standings,
    Feature::Wallet => state.wallet = !state.wallet,
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
