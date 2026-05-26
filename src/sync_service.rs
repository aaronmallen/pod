//! Background synchronisation service stub.
//!
//! `SyncService` will be expanded in later iterations to drive
//! periodic ESI fetches and push results into `DataStore`. The
//! `SyncRegistry` will track which characters and corporations
//! are enrolled for automatic refresh. For now both types are
//! minimal stubs so that the app lifecycle wiring can compile.

use std::time::Instant;

/// Events emitted by `SyncService` and routed through the iced
/// message loop.
#[derive(Clone, Debug)]
pub enum SyncEvent {
  /// A sync operation completed successfully.
  Completed {
    /// ESI data category that was synced (e.g. `"skills"`).
    character_id: i64,
    /// Timestamp at which the operation finished.
    completed_at: Instant,
    /// ESI data category that was synced (e.g. `"skills"`).
    data_type: String,
  },
  /// A sync operation failed with an ESI error.
  Error {
    /// Character the sync was attempted for.
    character_id: i64,
    /// ESI data category that failed.
    data_type: String,
    /// `true` when the failure was an ESI 5xx server error;
    /// `false` for rate-limit or client errors.
    is_server_error: bool,
  },
  /// A sync operation is currently in progress.
  InFlight {
    /// Character the sync is running for.
    character_id: i64,
    /// ESI data category currently being fetched.
    data_type: String,
  },
}

/// Tracks live sync activity for display in the status bar.
pub struct SyncRegistry {
  /// ESI data type names currently being fetched.
  pub in_flight: Vec<String>,
  /// Whether the most recent error was an ESI 5xx server error.
  pub has_server_error: bool,
  /// Timestamp of the most recent successful sync completion.
  pub last_synced_at: Option<Instant>,
}

impl SyncRegistry {
  /// Creates a new empty registry with no recorded activity.
  pub fn new() -> Self {
    Self {
      has_server_error: false,
      in_flight: Vec::new(),
      last_synced_at: None,
    }
  }

  /// Applies a `SyncEvent` to update the registry state.
  pub fn update(&mut self, event: SyncEvent) {
    match event {
      SyncEvent::Completed {
        data_type,
        completed_at,
        ..
      } => {
        self.in_flight.retain(|t| *t != data_type);
        self.last_synced_at = Some(completed_at);
      }
      SyncEvent::Error {
        data_type,
        is_server_error,
        ..
      } => {
        self.in_flight.retain(|t| *t != data_type);
        if is_server_error {
          self.has_server_error = true;
        }
      }
      SyncEvent::InFlight {
        data_type, ..
      } => {
        if !self.in_flight.contains(&data_type) {
          self.in_flight.push(data_type);
        }
      }
    }
  }
}

impl Default for SyncRegistry {
  fn default() -> Self {
    Self::new()
  }
}

/// Drives background ESI synchronisation for enrolled entities.
pub struct SyncService {}

impl SyncService {
  /// Creates a new `SyncService`.
  pub fn new() -> Self {
    Self {}
  }

  /// Requests an immediate re-sync of all enrolled entities.
  pub fn force_refresh_all(&self) {}

  /// Returns an iced subscription that emits `SyncEvent` items.
  pub fn subscription(&self) -> iced::Subscription<SyncEvent> {
    iced::Subscription::none()
  }
}
