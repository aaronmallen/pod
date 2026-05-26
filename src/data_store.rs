//! In-memory data store for shared application state.
//!
//! This module will be expanded in later iterations to hold
//! character data, asset records, and other views that the
//! SyncService keeps fresh. For now it is a minimal stub so
//! that downstream wiring can compile.

use pod_ui::views::{
  assets::AssetRecord,
  wallet::{ContractEntry, JournalEntry, MarketEntry},
};
/// Shared in-memory store populated by `SyncService`.
pub struct DataStore {}

impl DataStore {
  /// Loads (or initialises) the data store from persistent storage.
  pub fn load() -> Self {
    Self {}
  }

  /// Returns the asset records for the given character.
  ///
  /// Stub — `SyncService` will populate this in a later task.
  pub fn assets_for(&self, _character_id: i64) -> Vec<AssetRecord> {
    vec![]
  }

  /// Returns contracts for the given character.
  ///
  /// Returns an empty list until the SyncService populates this store.
  pub fn contracts_for(&self, _character_id: i64) -> Vec<ContractEntry> {
    vec![]
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

  /// Returns wallet journal entries for the given character.
  ///
  /// Returns an empty list until the SyncService populates this store.
  pub fn wallet_journal_for(&self, _character_id: i64) -> Vec<JournalEntry> {
    vec![]
  }

  /// Returns wallet market transactions for the given character.
  ///
  /// Returns an empty list until the SyncService populates this store.
  pub fn wallet_transactions_for(&self, _character_id: i64) -> Vec<MarketEntry> {
    vec![]
  }
}
