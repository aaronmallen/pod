//! Background synchronisation service stub.
//!
//! `SyncService` will be expanded in later iterations to drive
//! periodic ESI fetches and push results into `DataStore`. The
//! `SyncRegistry` will track which characters and corporations
//! are enrolled for automatic refresh. For now both types are
//! minimal stubs so that the app lifecycle wiring can compile.

/// Events emitted by `SyncService` and routed through the iced
/// message loop.
#[derive(Clone, Debug)]
pub enum SyncEvent {}

/// Tracks which entities are enrolled for background refresh.
pub struct SyncRegistry {}

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
