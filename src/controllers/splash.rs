//! Splash screen controller: bootstrap delegation.

use pod_model::Character;
pub use pod_ui::views::splash::{Message, State, update};

use crate::services::{bootstrap, sde};

/// Result of handling a bootstrap message; callers map each variant to the
/// appropriate top-level message type.
pub enum HandleResult {
  /// A bootstrap continuation task (maps to the top-level `Bootstrap` message).
  Bootstrap(iced::Task<bootstrap::Message>),
  /// Nothing to do.
  None,
  /// A splash animation task (maps to the top-level `Splash` message).
  Splash(iced::Task<Message>),
}

/// Handles a bootstrap message, applying side effects to app-level slots and
/// returning a continuation task for the caller to dispatch.
pub fn handle_bootstrap(
  state: &mut State,
  db: &mut Option<pod_db::Repo>,
  step_label: &mut String,
  characters: &mut Vec<Character>,
  esi_client: &mut Option<pod_esi::Client>,
  sync_step_size: &mut f32,
  msg: bootstrap::Message,
) -> HandleResult {
  match msg {
    bootstrap::Message::SeedingRequired(db_val) => {
      *db = Some(db_val.clone());
      *step_label = "Downloading static data\u{2026}".to_string();
      state.progress_target = 0.05;
      HandleResult::Bootstrap(sde::seed(db_val))
    }
    bootstrap::Message::SeedingComplete(db_val) => {
      *db = Some(db_val.clone());
      *step_label = "Loading characters\u{2026}".to_string();
      state.progress_target = 0.85;
      HandleResult::Bootstrap(bootstrap::continue_after_db(db_val))
    }
    bootstrap::Message::SyncStarted(total_steps) => {
      if total_steps > 0 {
        *sync_step_size = (1.0 - state.progress_target) / total_steps as f32;
      }
      HandleResult::None
    }
    bootstrap::Message::StepChanged(label) => {
      *step_label = label;
      state.progress_target = (state.progress_target + *sync_step_size).min(1.0);
      HandleResult::None
    }
    bootstrap::Message::Complete(db_val, chars, esi) => {
      *db = Some(db_val);
      *characters = chars;
      *esi_client = esi;
      state.progress_target = 1.0;
      HandleResult::Splash(update(state, Message::LoadingComplete))
    }
    bootstrap::Message::Error(e) => {
      log::error!("bootstrap error: {e}");
      eprintln!("Bootstrap error: {e}");
      state.progress_target = 1.0;
      HandleResult::Splash(update(state, Message::LoadingComplete))
    }
  }
}
