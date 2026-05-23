//! Asset sync state domain model.

/// Tracks the last ESI sync time and cache expiry for one asset owner
/// (character or corporation). Used to avoid re-fetching ESI data before
/// the cache window has elapsed.
#[derive(Clone, Debug)]
pub struct Model {
  /// Unix timestamp after which the cached ESI response is considered stale.
  pub cache_expires_at: Option<i64>,
  /// Unix timestamp of the most recent successful sync.
  pub last_synced_at: Option<i64>,
  /// EVE ID of the owning character or corporation.
  pub owner_id: i64,
  /// `"character"` or `"corporation"`.
  pub owner_type: String,
}
