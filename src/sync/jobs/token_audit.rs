//! Global, ungated token audit: every cycle it re-validates every stored credential and re-checks
//! its granted scopes against what the currently-enabled features require, persisting a
//! `needs_reauth` flag (and clearing a stale one) so the engine can park a dead/under-scoped entity
//! instead of hammering its token forever.
//!
//! Two independent signals mark an entity:
//!   1. validity — a refresh against SSO fails with a revoked-token signal (HTTP 400 / invalid_grant);
//!   2. scope sufficiency — the granted scopes miss a scope a currently-enabled feature requires.
//!
//! If *either* fails the entity is flagged; only when *both* pass is an existing flag cleared, so the
//! audit self-heals a token that became healthy again (re-authorized out of band, or a feature
//! disabled). A corporation is additionally flagged when its authorizing director's character token
//! is bad (the director cascade), since a corp's jobs run on the director's grant.

use std::collections::HashSet;

use crate::{
  clients::{Error, eve_sso},
  config,
  features::{auth, character_manager},
  store::{
    Database,
    model::{Credential, OwnerType},
    repo::infra,
  },
  sync::{job::JobCtx, outcome::Outcome, token},
};

pub async fn run(ctx: &JobCtx<'_>) -> Result<Outcome, Error> {
  let Some(sso) = ctx.sso else {
    // A global job is always dispatched with the SSO client; its absence means a caller built the
    // ctx wrong rather than a recoverable condition, so surface it loudly but harmlessly.
    tracing::warn!("token audit dispatched without an SSO client; skipping this cycle");
    return Ok(Outcome::Skipped {
      reason: "no SSO client".to_string(),
    });
  };
  let enabled = config::load()
    .map(|settings| settings.features().enabled())
    .unwrap_or_default();
  let credentials = infra::all(ctx.db).await?;
  Ok(audit(ctx.db, sso, &credentials, &enabled).await)
}

/// Audits the supplied credentials and returns how many were flagged this pass. Split out from
/// [`run`] so tests can drive it with an explicit credential set and feature list rather than the
/// on-disk config and the full engine dispatch.
pub async fn audit(
  db: &Database,
  sso: &eve_sso::Client,
  credentials: &[Credential],
  enabled_features: &[config::Feature],
) -> Outcome {
  // First pass: learn which character tokens are dead, so the second pass can cascade a dead
  // director's revocation onto the corporations it authorizes.
  let mut dead_characters: HashSet<i64> = HashSet::new();
  for credential in credentials {
    if credential.owner_type() == OwnerType::Character && is_token_dead(db, sso, credential).await {
      dead_characters.insert(credential.owner_id());
    }
  }

  let mut flagged = 0usize;
  for credential in credentials {
    let owner_id = credential.owner_id();
    let owner_type = credential.owner_type();
    let healthy = match owner_type {
      OwnerType::Character => !dead_characters.contains(&owner_id) && scopes_sufficient(credential, enabled_features),
      OwnerType::Corporation => {
        let director_dead = credential
          .authorized_by()
          .is_some_and(|director| dead_characters.contains(&director));
        let token_dead = is_token_dead(db, sso, credential).await;
        !director_dead && !token_dead && scopes_sufficient(credential, enabled_features)
      }
    };

    if healthy {
      if credential.needs_reauth() {
        clear(db, owner_id, owner_type).await;
      }
    } else {
      flagged += 1;
      if !credential.needs_reauth() {
        mark(db, owner_id, owner_type).await;
      }
    }
  }

  Outcome::from_rows(flagged)
}

/// True when the credential's granted scopes already cover everything the enabled features require.
fn scopes_sufficient(credential: &Credential, enabled_features: &[config::Feature]) -> bool {
  let required = match credential.owner_type() {
    OwnerType::Character => auth::scopes_for(enabled_features),
    OwnerType::Corporation => auth::corp_scopes_for(enabled_features),
  };
  !character_manager::needs_reauthorization(credential.scopes().as_deref(), &required)
}

/// True only when SSO positively rejects the refresh as revoked; a transient failure leaves the
/// token presumed-alive so a network blip never parks a healthy entity.
async fn is_token_dead(db: &Database, sso: &eve_sso::Client, credential: &Credential) -> bool {
  match token::validate_credential(db, sso, credential.owner_id(), credential.owner_type()).await {
    Ok(valid) => !valid,
    Err(error) => {
      tracing::debug!(
        owner_id = credential.owner_id(),
        ?error,
        "token validity check failed transiently; presuming alive this cycle"
      );
      false
    }
  }
}

async fn mark(db: &Database, owner_id: i64, owner_type: OwnerType) {
  match infra::mark_needs_reauth(db, owner_id, owner_type).await {
    Ok(()) => tracing::info!(owner_id, ?owner_type, "token audit flagged entity as needing re-auth"),
    Err(error) => tracing::warn!(owner_id, ?owner_type, %error, "failed to flag entity as needing re-auth"),
  }
}

async fn clear(db: &Database, owner_id: i64, owner_type: OwnerType) {
  match infra::clear_needs_reauth(db, owner_id, owner_type).await {
    Ok(()) => tracing::info!(owner_id, ?owner_type, "token audit cleared a stale needs-reauth flag"),
    Err(error) => tracing::warn!(owner_id, ?owner_type, %error, "failed to clear stale needs-reauth flag"),
  }
}

#[cfg(test)]
mod tests {
  use base64::Engine as _;
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{clients::http, config::Feature, store};

  fn jwt_access(character_id: i64) -> String {
    let encode = |raw: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw);
    format!(
      "{}.{}.sig",
      encode(r#"{"alg":"RS256","typ":"JWT"}"#),
      encode(&format!(
        r#"{{"sub":"CHARACTER:EVE:{character_id}","name":"Pilot","scp":[]}}"#
      )),
    )
  }

  async fn mount_live_token(server: &MockServer, character_id: i64) {
    let access = jwt_access(character_id);
    let body = format!(r#"{{"access_token":"{access}","expires_in":1200,"refresh_token":"rotated-rt"}}"#);
    Mock::given(method("POST"))
      .and(path("/token"))
      .respond_with(ResponseTemplate::new(200).set_body_raw(body.into_bytes(), "application/json"))
      .mount(server)
      .await;
  }

  async fn mount_revoked_token(server: &MockServer) {
    Mock::given(method("POST"))
      .and(path("/token"))
      .respond_with(
        ResponseTemplate::new(400).set_body_raw(br#"{"error":"invalid_grant"}"#.to_vec(), "application/json"),
      )
      .mount(server)
      .await;
  }

  fn sso_for(server: &MockServer, db: &Database) -> eve_sso::Client {
    let http = http::Client::builder(http::Cache::new(db.clone())).build();
    eve_sso::Client::new(http, "test-client").with_token_url(format!("{}/token", server.uri()))
  }

  // A scope every enabled feature is known to need, so a credential lacking it is under-scoped.
  fn a_required_character_scope() -> String {
    auth::scopes_for(&Feature::ALL)
      .first()
      .expect("the full feature set requires at least one character scope")
      .to_string()
  }

  #[tokio::test]
  async fn it_cascades_a_dead_director_onto_its_corporation() {
    let server = MockServer::start().await;
    // The director (and corp, which shares the token path) refresh against the same revoked endpoint.
    mount_revoked_token(&server).await;
    let db = store::open_test().await.unwrap();
    let sso = sso_for(&server, &db);
    let far_future = chrono::Utc::now().timestamp() + 86_400;
    let char_scopes = auth::scopes_for(&Feature::ALL).join(" ");
    let corp_scopes = auth::corp_scopes_for(&Feature::ALL).join(" ");
    // Director character 400 authorizes corporation 9000.
    infra::upsert(
      &db,
      400,
      OwnerType::Character,
      "at",
      "rt",
      far_future,
      None,
      Some(&char_scopes),
    )
    .await
    .unwrap();
    infra::upsert(
      &db,
      9000,
      OwnerType::Corporation,
      "at",
      "rt",
      far_future,
      Some(400),
      Some(&corp_scopes),
    )
    .await
    .unwrap();

    let credentials = infra::all(&db).await.unwrap();
    let outcome = audit(&db, &sso, &credentials, &Feature::ALL).await;

    assert_eq!(
      outcome,
      Outcome::Synced {
        rows_touched: 2
      },
      "both director and corp flag"
    );
    let corp = infra::get(&db, 9000, OwnerType::Corporation).await.unwrap().unwrap();
    assert!(
      corp.needs_reauth(),
      "a corporation whose authorizing director's token is dead must itself be flagged"
    );
  }

  #[tokio::test]
  async fn it_clears_a_stale_flag_when_validity_and_scopes_are_healthy() {
    let server = MockServer::start().await;
    mount_live_token(&server, 300).await;
    let db = store::open_test().await.unwrap();
    let sso = sso_for(&server, &db);
    let far_future = chrono::Utc::now().timestamp() + 86_400;
    let scopes = auth::scopes_for(&Feature::ALL).join(" ");
    infra::upsert(
      &db,
      300,
      OwnerType::Character,
      "at",
      "rt",
      far_future,
      None,
      Some(&scopes),
    )
    .await
    .unwrap();
    infra::mark_needs_reauth(&db, 300, OwnerType::Character).await.unwrap();

    let credentials = infra::all(&db).await.unwrap();
    let outcome = audit(&db, &sso, &credentials, &Feature::ALL).await;

    assert_eq!(outcome, Outcome::Empty, "a healthy entity flags nobody");
    let after = infra::get(&db, 300, OwnerType::Character).await.unwrap().unwrap();
    assert!(
      !after.needs_reauth(),
      "a fully-healthy token self-heals: the stale flag is cleared"
    );
  }

  #[tokio::test]
  async fn it_flags_a_character_missing_a_required_scope() {
    let server = MockServer::start().await;
    mount_live_token(&server, 200).await;
    let db = store::open_test().await.unwrap();
    let sso = sso_for(&server, &db);
    let far_future = chrono::Utc::now().timestamp() + 86_400;
    // A live token, but its granted scopes are empty while features require some.
    infra::upsert(&db, 200, OwnerType::Character, "at", "rt", far_future, None, Some(""))
      .await
      .unwrap();

    let credentials = infra::all(&db).await.unwrap();
    let outcome = audit(&db, &sso, &credentials, &Feature::ALL).await;

    assert_eq!(
      outcome,
      Outcome::Synced {
        rows_touched: 1
      }
    );
    let after = infra::get(&db, 200, OwnerType::Character).await.unwrap().unwrap();
    assert!(
      after.needs_reauth(),
      "a live but under-scoped token must mark the character needs-reauth"
    );
  }

  #[tokio::test]
  async fn it_flags_a_revoked_character_token() {
    let server = MockServer::start().await;
    mount_revoked_token(&server).await;
    let db = store::open_test().await.unwrap();
    let sso = sso_for(&server, &db);
    // Far-future expiry: the token is not near expiry, so only a forced refresh reveals revocation.
    let far_future = chrono::Utc::now().timestamp() + 86_400;
    let scopes = a_required_character_scope();
    infra::upsert(
      &db,
      100,
      OwnerType::Character,
      "at",
      "rt",
      far_future,
      None,
      Some(&scopes),
    )
    .await
    .unwrap();

    let credentials = infra::all(&db).await.unwrap();
    let outcome = audit(&db, &sso, &credentials, &Feature::ALL).await;

    assert_eq!(
      outcome,
      Outcome::Synced {
        rows_touched: 1
      },
      "the revoked token is flagged"
    );
    let after = infra::get(&db, 100, OwnerType::Character).await.unwrap().unwrap();
    assert!(
      after.needs_reauth(),
      "a revoked refresh token must mark the character needs-reauth"
    );
  }
}
