//! Corporation service: token management for corp-authenticated ESI calls.

use pod_esi::models::auth::Grant;
use pod_model::Corporation;

/// Ensures the corporation's access token is valid, refreshing via ESI if expired.
///
/// Returns `Some(token)` on success or `None` if the refresh failed (caller
/// should skip the corporation silently).
pub async fn ensure_valid_token(corp: &Corporation, esi: &pod_esi::Client, db: &pod_db::Repo) -> Option<String> {
  if !corp.access_token_expired() {
    return Some(corp.access_token().clone());
  }

  let expires_at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(*corp.token_expires_at() as u64);
  let grant = Grant::new(
    corp.access_token().clone(),
    *corp.auth_character_id(),
    String::new(),
    expires_at,
    corp.refresh_token().clone(),
    corp.scopes().clone(),
  );

  let new_grant = esi.auth().refresh(&grant).await.ok()?;

  let new_expires_at = new_grant
    .expires_at()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);

  db.corporations()
    .update_token(
      *corp.id(),
      new_grant.access_token(),
      new_grant.refresh_token(),
      new_expires_at,
    )
    .await
    .ok()?;

  Some(new_grant.access_token().clone())
}

/// Constructs a refreshed `Grant` for a corporation using the given access token.
pub fn refresh_grant(corp: &Corporation, access_token: &str) -> Grant {
  let expires_at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(*corp.token_expires_at() as u64);
  Grant::new(
    access_token,
    *corp.auth_character_id(),
    String::new(),
    expires_at,
    corp.refresh_token().clone(),
    corp.scopes().clone(),
  )
}
