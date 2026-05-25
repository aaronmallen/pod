//! Splash screen controller: bootstrap delegation.

use pod_model::Character;
pub use pod_ui::views::splash::{Message, State, update};

use crate::services::{bootstrap, sde};

/// Result of handling a bootstrap message; callers map each variant to the
/// appropriate top-level message type.
pub enum HandleResult {
  /// A bootstrap continuation task (maps to the top-level `Bootstrap` message).
  Bootstrap(iced::Task<bootstrap::Message>),
  /// A fatal error that the app cannot recover from; carries the error string.
  Fatal(String),
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
  msg: bootstrap::Message,
) -> HandleResult {
  match msg {
    bootstrap::Message::SeedingRequired(db_val) => {
      *db = Some(db_val.clone());
      *step_label = "Downloading static data\u{2026}".to_string();
      state.progress_target = 0.25;
      HandleResult::Bootstrap(sde::seed(db_val))
    }
    bootstrap::Message::SeedingComplete(db_val) => {
      *db = Some(db_val.clone());
      state.progress_target = 0.50;
      HandleResult::Bootstrap(bootstrap::continue_after_db(db_val))
    }
    bootstrap::Message::CharacterSynced(_) | bootstrap::Message::TokenRefreshFailed(_) => HandleResult::None,
    bootstrap::Message::StepChanged(label) => apply_step_progress(state, step_label, label),
    msg => handle_bootstrap_ext(state, db, characters, esi_client, msg),
  }
}

fn apply_step_progress(state: &mut State, step_label: &mut String, label: String) -> HandleResult {
  *step_label = label;
  if state.progress_target >= 0.50 {
    state.progress_target = (state.progress_target + 0.25).min(1.0);
  }
  HandleResult::None
}

fn handle_bootstrap_ext(
  state: &mut State,
  db: &mut Option<pod_db::Repo>,
  characters: &mut Vec<Character>,
  esi_client: &mut Option<pod_esi::Client>,
  msg: bootstrap::Message,
) -> HandleResult {
  match msg {
    bootstrap::Message::Complete(db_val, chars, esi) => {
      *db = Some(db_val);
      *characters = chars;
      *esi_client = esi;
      state.progress_target = 1.0;
      HandleResult::Splash(update(state, Message::LoadingComplete))
    }
    bootstrap::Message::Error(e) => {
      tracing::error!("bootstrap error: {e}");
      state.progress_target = 1.0;
      HandleResult::Splash(update(state, Message::LoadingComplete))
    }
    bootstrap::Message::FatalError(e) => {
      tracing::error!("fatal bootstrap error: {e}");
      HandleResult::Fatal(e)
    }
    _ => HandleResult::None,
  }
}
