//! In-memory data store for shared application state.
//!
//! This module will be expanded in later iterations to hold
//! character data, asset records, and other views that the
//! SyncService keeps fresh. For now it is a minimal stub so
//! that downstream wiring can compile.

/// Shared in-memory store populated by `SyncService`.
pub struct DataStore {}

impl DataStore {
  /// Loads (or initialises) the data store from persistent storage.
  pub fn load() -> Self {
    Self {}
  }
}
