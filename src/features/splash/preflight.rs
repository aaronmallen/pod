use std::time::Duration;

use iced::Task;

use crate::{features::splash::Message, services::updater};

pub const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
  Failed(String),
  NoUpdate,
  UpdateAvailable { version: String },
}

impl Outcome {
  fn from_state(state: &updater::State) -> Option<Self> {
    match state {
      updater::State::Error {
        message,
      } => Some(Self::Failed(message.clone())),
      available @ updater::State::UpdateAvailable {
        ..
      } => available.version().map(|version| Self::UpdateAvailable {
        version: version.to_owned(),
      }),
      updater::State::Downloading {
        ..
      }
      | updater::State::Idle
      | updater::State::ReadyToRestart {
        ..
      } => None,
    }
  }

  fn into_message(self) -> Message {
    match self {
      Self::Failed(error) => Message::UpdateFailed(error),
      Self::NoUpdate => Message::UpdateNotAvailable,
      Self::UpdateAvailable {
        version,
      } => Message::UpdateAvailable(version),
    }
  }
}

pub fn check(handle: &updater::Handle) -> Task<Message> {
  let mut state = handle.subscribe();
  let mut checks = handle.subscribe_checks();
  let baseline = *checks.borrow();
  handle.check();

  Task::future(async move {
    let outcome = tokio::time::timeout(TIMEOUT, async {
      loop {
        if let Some(outcome) = Outcome::from_state(&state.borrow_and_update()) {
          return outcome;
        }
        if *checks.borrow() != baseline {
          return Outcome::NoUpdate;
        }
        tokio::select! {
          changed = state.changed() => {
            if changed.is_err() {
              return Outcome::NoUpdate;
            }
          }
          changed = checks.changed() => {
            if changed.is_err() {
              return Outcome::NoUpdate;
            }
          }
        }
      }
    })
    .await
    .unwrap_or(Outcome::NoUpdate);

    outcome.into_message()
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  mod outcome {
    use super::*;

    mod from_state {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_maps_an_available_update_to_update_available() {
        let state = updater::State::UpdateAvailable {
          version: "9.9.9".to_string(),
        };

        assert_eq!(
          Outcome::from_state(&state),
          Some(Outcome::UpdateAvailable {
            version: "9.9.9".to_string()
          })
        );
      }

      #[test]
      fn it_maps_an_error_to_failed() {
        let state = updater::State::Error {
          message: "check boom".to_string(),
        };

        assert_eq!(
          Outcome::from_state(&state),
          Some(Outcome::Failed("check boom".to_string()))
        );
      }

      #[test]
      fn it_treats_idle_as_unresolved() {
        assert_eq!(Outcome::from_state(&updater::State::Idle), None);
      }

      #[test]
      fn it_treats_downloading_as_unresolved() {
        let state = updater::State::Downloading {
          version: "9.9.9".to_string(),
        };

        assert_eq!(Outcome::from_state(&state), None);
      }

      #[test]
      fn it_treats_ready_to_restart_as_unresolved() {
        let state = updater::State::ReadyToRestart {
          version: "9.9.9".to_string(),
        };

        assert_eq!(Outcome::from_state(&state), None);
      }
    }

    mod into_message {
      use super::*;

      #[test]
      fn it_maps_failed_to_update_failed() {
        let message = Outcome::Failed("boom".to_string()).into_message();

        assert!(matches!(message, Message::UpdateFailed(error) if error == "boom"));
      }

      #[test]
      fn it_maps_no_update_to_update_not_available() {
        let message = Outcome::NoUpdate.into_message();

        assert!(matches!(message, Message::UpdateNotAvailable));
      }

      #[test]
      fn it_maps_update_available_to_update_available_message() {
        let message = Outcome::UpdateAvailable {
          version: "9.9.9".to_string(),
        }
        .into_message();

        assert!(matches!(message, Message::UpdateAvailable(version) if version == "9.9.9"));
      }
    }
  }
}
