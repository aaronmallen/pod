//! Sync status model and error types.
//!
//! Defines the types used to track and communicate synchronisation status
//! throughout the application. [`SyncRegistry`] is the central map that the
//! main window state holds and updates as [`SyncEvent`]s arrive from
//! `SyncService`.

use std::{collections::HashMap, time::SystemTime};

use serde::{Deserialize, Serialize};

/// Identifies a category of ESI data being synced per character.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum SyncDataType {
  /// Character assets list (`esi-assets.read_assets`).
  CharacterAssets,
  /// Character location (`esi-location.read_location`).
  CharacterLocation,
  /// Character contracts (`esi-contracts.read_character_contracts`).
  Contracts,
  /// Corporation assets list (`esi-assets.read_corporation_assets`).
  CorpAssets,
  /// Corporation wallet balance (`esi-wallet.read_corporation_wallets`).
  CorpWallet,
  /// Character mail (`esi-mail.read_mail`).
  Mail,
  /// Character skill queue and trained skills (`esi-skills.read_skills`).
  Skills,
  /// Character wallet balance (`esi-wallet.read_character_wallet`).
  WalletBalance,
  /// Character wallet journal (`esi-wallet.read_character_wallet`).
  WalletJournal,
  /// Character wallet transactions (`esi-wallet.read_character_wallet`).
  WalletTransactions,
}

/// Tracks state for one data type for one character.
#[derive(Clone, Debug)]
pub struct SyncEntry {
  /// When this endpoint last synced successfully.
  pub last_synced_at: Option<SystemTime>,
  /// When the next sync for this endpoint is allowed.
  pub next_allowed_at: Option<SystemTime>,
  /// Current sync status for this entry.
  pub status: SyncStatus,
}

impl SyncEntry {
  /// Creates a new [`SyncEntry`] with [`SyncStatus::Idle`] and no timestamps.
  pub fn new() -> Self {
    Self {
      last_synced_at: None,
      next_allowed_at: None,
      status: SyncStatus::Idle,
    }
  }
}

impl Default for SyncEntry {
  fn default() -> Self {
    Self::new()
  }
}

/// An error that occurred during a sync operation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum SyncError {
  /// ESI returned a 401/403 response; the user must re-authenticate.
  ///
  /// The payload is the `character_id` that needs re-auth.
  AuthExpired(u64),
  /// ESI returned a 5xx server error; a persistent badge is shown in the
  /// status bar until the next successful sync.
  EsiServerError(u16),
  /// Any other error not covered by the variants above.
  Other(String),
  /// ESI returned a 420 or 429 rate-limit response; the error is silenced
  /// and the endpoint is retried after a back-off period.
  RateLimit,
}

impl SyncError {
  /// Returns `true` when the error should be hidden from the user.
  ///
  /// Rate-limit responses are transient; surfacing them would be noisy
  /// because the scheduler will retry automatically after the back-off.
  pub fn is_silent(&self) -> bool {
    matches!(self, Self::RateLimit)
  }
}

/// Events emitted by `SyncService` into the iced message stream.
#[derive(Clone, Debug)]
pub enum SyncEvent {
  /// A sync operation completed successfully.
  Completed {
    /// Character whose data was synced.
    character_id: u64,
    /// The ESI data category that was synced.
    data_type: SyncDataType,
    /// When the next sync for this endpoint is allowed (from `x-cached-seconds`).
    next_allowed_at: SystemTime,
  },
  /// A sync operation failed.
  Failed {
    /// Character the sync was attempted for.
    character_id: u64,
    /// The ESI data category that failed.
    data_type: SyncDataType,
    /// The error that caused the failure.
    error: SyncError,
  },
  /// A sync operation is in progress.
  Started {
    /// Character the sync is running for.
    character_id: u64,
    /// The ESI data category being fetched.
    data_type: SyncDataType,
  },
}

/// Map of `(SyncDataType, character_id)` to [`SyncEntry`].
///
/// The main window state holds a `SyncRegistry` and calls [`SyncRegistry::update`]
/// for every [`SyncEvent`] that arrives from `SyncService`.
#[derive(Clone, Debug, Default)]
pub struct SyncRegistry {
  entries: HashMap<RegistryKey, SyncEntry>,
}

impl SyncRegistry {
  /// Creates an empty [`SyncRegistry`].
  pub fn new() -> Self {
    Self {
      entries: HashMap::new(),
    }
  }

  /// Applies a [`SyncEvent`] to update the registry.
  ///
  /// - [`SyncEvent::Started`] marks the entry as [`SyncStatus::InFlight`].
  /// - [`SyncEvent::Completed`] marks the entry as [`SyncStatus::Idle`] and
  ///   records `last_synced_at` and `next_allowed_at`.
  /// - [`SyncEvent::Failed`] marks the entry as [`SyncStatus::Error`].
  pub fn update(&mut self, event: SyncEvent) {
    match event {
      SyncEvent::Completed {
        character_id,
        data_type,
        next_allowed_at,
      } => {
        let key = RegistryKey {
          character_id,
          data_type,
        };
        let entry = self.entries.entry(key).or_default();
        entry.last_synced_at = Some(SystemTime::now());
        entry.next_allowed_at = Some(next_allowed_at);
        entry.status = SyncStatus::Idle;
      }
      SyncEvent::Failed {
        character_id,
        data_type,
        error,
      } => {
        let key = RegistryKey {
          character_id,
          data_type,
        };
        let entry = self.entries.entry(key).or_default();
        entry.status = SyncStatus::Error(error);
      }
      SyncEvent::Started {
        character_id,
        data_type,
      } => {
        let key = RegistryKey {
          character_id,
          data_type,
        };
        let entry = self.entries.entry(key).or_default();
        entry.status = SyncStatus::InFlight;
      }
    }
  }
}

/// The sync lifecycle state for a single `(SyncDataType, character_id)` pair.
#[derive(Clone, Debug, PartialEq)]
pub enum SyncStatus {
  /// The endpoint encountered an error on its last attempt.
  Error(SyncError),
  /// A sync is currently in progress.
  InFlight,
  /// No sync is running; waiting for the next scheduled tick.
  Idle,
}

/// Key identifying a single `(data_type, character_id)` polling slot.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RegistryKey {
  character_id: u64,
  data_type: SyncDataType,
}

#[cfg(test)]
mod tests {
  use super::*;

  mod sync_error {
    use super::*;

    mod is_silent {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_true_for_rate_limit() {
        assert_eq!(SyncError::RateLimit.is_silent(), true);
      }

      #[test]
      fn it_returns_false_for_auth_expired() {
        assert_eq!(SyncError::AuthExpired(1234).is_silent(), false);
      }

      #[test]
      fn it_returns_false_for_esi_server_error() {
        assert_eq!(SyncError::EsiServerError(500).is_silent(), false);
      }

      #[test]
      fn it_returns_false_for_other() {
        assert_eq!(SyncError::Other("boom".to_string()).is_silent(), false);
      }
    }
  }

  mod sync_registry {
    use super::*;

    mod update {
      use super::*;

      #[test]
      fn it_marks_entry_as_in_flight_on_started() {
        let mut reg = SyncRegistry::new();

        reg.update(SyncEvent::Started {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
        });

        let key = RegistryKey {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
        };
        assert_eq!(reg.entries[&key].status, SyncStatus::InFlight);
      }

      #[test]
      fn it_marks_entry_as_idle_and_records_timestamps_on_completed() {
        let mut reg = SyncRegistry::new();
        let next = SystemTime::now();

        reg.update(SyncEvent::Completed {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
          next_allowed_at: next,
        });

        let key = RegistryKey {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
        };
        assert_eq!(reg.entries[&key].status, SyncStatus::Idle);
        assert!(reg.entries[&key].last_synced_at.is_some());
        assert!(reg.entries[&key].next_allowed_at.is_some());
      }

      #[test]
      fn it_marks_entry_as_error_on_failed() {
        let mut reg = SyncRegistry::new();

        reg.update(SyncEvent::Failed {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
          error: SyncError::EsiServerError(503),
        });

        let key = RegistryKey {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
        };
        assert_eq!(
          reg.entries[&key].status,
          SyncStatus::Error(SyncError::EsiServerError(503))
        );
      }

      #[test]
      fn it_tracks_multiple_characters_independently() {
        let mut reg = SyncRegistry::new();

        reg.update(SyncEvent::Started {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
        });
        reg.update(SyncEvent::Started {
          character_id: 2,
          data_type: SyncDataType::WalletBalance,
        });

        let key1 = RegistryKey {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
        };
        let key2 = RegistryKey {
          character_id: 2,
          data_type: SyncDataType::WalletBalance,
        };
        assert_eq!(reg.entries[&key1].status, SyncStatus::InFlight);
        assert_eq!(reg.entries[&key2].status, SyncStatus::InFlight);
      }

      #[test]
      fn it_tracks_multiple_data_types_independently() {
        let mut reg = SyncRegistry::new();

        reg.update(SyncEvent::Started {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
        });
        reg.update(SyncEvent::Started {
          character_id: 1,
          data_type: SyncDataType::Skills,
        });

        let key_wallet = RegistryKey {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
        };
        let key_skills = RegistryKey {
          character_id: 1,
          data_type: SyncDataType::Skills,
        };
        assert_eq!(reg.entries[&key_wallet].status, SyncStatus::InFlight);
        assert_eq!(reg.entries[&key_skills].status, SyncStatus::InFlight);
      }

      #[test]
      fn it_does_not_preserve_silent_error_in_is_silent_check() {
        let mut reg = SyncRegistry::new();

        reg.update(SyncEvent::Failed {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
          error: SyncError::RateLimit,
        });

        let key = RegistryKey {
          character_id: 1,
          data_type: SyncDataType::WalletBalance,
        };
        let SyncStatus::Error(err) = &reg.entries[&key].status else {
          panic!("expected Error status");
        };
        assert!(err.is_silent());
      }
    }
  }
}
