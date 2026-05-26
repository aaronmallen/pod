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

  /// Returns cached mail messages for the given character.
  ///
  /// Always empty until SyncService populates the store.
  pub fn mail_for(&self, _character_id: i64) -> Vec<pod_ui::views::mail::MailMessage> {
    vec![]
  }

  /// Returns skill group definitions for the given character.
  ///
  /// Always returns an empty slice until the SyncService populates
  /// the store with universe skill data.
  pub fn skills_for(&self, _character_id: i64) -> Vec<pod_model::SkillGroupDef> {
    vec![]
  }
}
