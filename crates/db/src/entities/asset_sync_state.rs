//! Database entity for asset sync state tracking.

use pod_model::AssetSyncState;
use sea_orm::prelude::*;

/// A row in the `asset_sync_state` table tracking the last ESI sync time
/// and cache expiry for one asset owner (character or corporation).
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "asset_sync_state")]
pub struct Model {
  /// Unix timestamp after which the cached ESI response is considered stale.
  pub cache_expires_at: Option<i64>,
  /// Unix timestamp of the most recent successful sync.
  pub last_synced_at: Option<i64>,
  /// EVE ID of the owning character or corporation.
  #[sea_orm(primary_key, auto_increment = false)]
  pub owner_id: i64,
  /// `"character"` or `"corporation"`.
  #[sea_orm(primary_key, auto_increment = false)]
  pub owner_type: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for AssetSyncState {
  fn from(m: Model) -> Self {
    Self {
      cache_expires_at: m.cache_expires_at,
      last_synced_at: m.last_synced_at,
      owner_id: m.owner_id,
      owner_type: m.owner_type,
    }
  }
}
