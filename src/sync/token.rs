#[cfg(test)]
use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::{
  clients::{self, eve_sso, eve_sso::Grant},
  store::{Database, model::OwnerType, repo::infra},
};

const REFRESH_SKEW_SECS: i64 = 60;

#[cfg(test)]
#[derive(Default)]
pub struct TokenCache {
  entries: HashMap<(i64, u8), Option<Grant>>,
}

#[cfg(test)]
impl TokenCache {
  pub fn new() -> Self {
    Self::default()
  }

  pub async fn get(
    &mut self,
    db: &Database,
    sso: &eve_sso::Client,
    owner_id: i64,
    owner_type: OwnerType,
  ) -> Result<Option<Grant>, clients::Error> {
    let key = (owner_id, owner_key(owner_type));
    if let Some(cached) = self.entries.get(&key) {
      tracing::trace!(owner_id, ?owner_type, "reusing cached grant for this cycle");
      return Ok(cached.clone());
    }
    let grant = fresh_token(db, sso, owner_id, owner_type).await?;
    tracing::debug!(
      owner_id,
      ?owner_type,
      obtained = grant.is_some(),
      "resolved subject grant once for this cycle"
    );
    self.entries.insert(key, grant.clone());
    Ok(grant)
  }
}

/// Returns the stored token, or refreshes it via SSO and persists the rotated credential when it is near expiry.
pub async fn fresh_token(
  db: &Database,
  sso: &eve_sso::Client,
  owner_id: i64,
  owner_type: OwnerType,
) -> Result<Option<Grant>, clients::Error> {
  let Some(credential) = infra::get(db, owner_id, owner_type).await? else {
    return Ok(None);
  };

  if !needs_refresh(credential.expires_at(), Utc::now().timestamp(), REFRESH_SKEW_SECS) {
    let expires_at = DateTime::from_timestamp(credential.expires_at(), 0).unwrap_or_else(Utc::now);
    let scopes = credential
      .scopes()
      .as_deref()
      .map(|s| s.split_whitespace().map(str::to_owned).collect())
      .unwrap_or_default();
    return Ok(Some(Grant::from_stored(
      credential.access_token(),
      owner_id,
      expires_at,
      credential.refresh_token(),
      scopes,
    )));
  }

  let grant = sso.refresh_with_token(credential.refresh_token()).await?;
  let scopes = grant.scopes().join(" ");
  infra::upsert(
    db,
    owner_id,
    owner_type,
    grant.access_token(),
    grant.refresh_token(),
    grant.expires_at().timestamp(),
    credential.authorized_by(),
    Some(&scopes),
  )
  .await?;
  Ok(Some(grant))
}

/// Forces a refresh against SSO to prove a credential's refresh token is still live, persisting the
/// rotated token on success. Unlike [`fresh_token`], it does not short-circuit on a not-yet-expired
/// token: a token can be revoked at the SSO long before it would naturally expire, and the only way
/// to learn that is to actually exercise the refresh. Returns `Ok(true)` when the token is valid,
/// `Ok(false)` when SSO rejects the refresh (a revoked/dead token), and `Err` for transient failures
/// (network, rate limits) that must not be mistaken for revocation.
pub async fn validate_credential(
  db: &Database,
  sso: &eve_sso::Client,
  owner_id: i64,
  owner_type: OwnerType,
) -> Result<bool, clients::Error> {
  let Some(credential) = infra::get(db, owner_id, owner_type).await? else {
    return Ok(false);
  };
  match sso.refresh_with_token(credential.refresh_token()).await {
    Ok(grant) => {
      let scopes = grant.scopes().join(" ");
      infra::upsert(
        db,
        owner_id,
        owner_type,
        grant.access_token(),
        grant.refresh_token(),
        grant.expires_at().timestamp(),
        credential.authorized_by(),
        Some(&scopes),
      )
      .await?;
      Ok(true)
    }
    Err(error) if is_revoked_refresh(&error) => Ok(false),
    Err(error) => Err(error),
  }
}

/// A refresh that fails with HTTP 400 (`invalid_grant`) is the EVE SSO signal for a revoked or
/// otherwise dead refresh token; any other error (401/403/5xx/network) is transient and must not be
/// read as revocation, lest a blip permanently parks a healthy entity.
pub fn is_revoked_refresh(error: &clients::Error) -> bool {
  matches!(error, clients::Error::Http(http) if http.status() == Some(reqwest::StatusCode::BAD_REQUEST))
}

fn needs_refresh(expires_at: i64, now: i64, skew: i64) -> bool {
  expires_at - skew <= now
}

#[cfg(test)]
fn owner_key(owner_type: OwnerType) -> u8 {
  match owner_type {
    OwnerType::Character => 0,
    OwnerType::Corporation => 1,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{clients::http, store};

  async fn make_sso() -> eve_sso::Client {
    let db = store::open_test().await.unwrap();
    let http = http::Client::builder(http::Cache::new(db)).build();
    eve_sso::Client::new(http, "test-client")
  }

  mod fresh_token {
    use super::*;

    #[tokio::test]
    async fn it_preserves_the_authorizing_character_when_refreshing_a_corporation_credential() {
      use base64::Engine as _;
      use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
      };

      let server = MockServer::start().await;
      let encode = |raw: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw);
      let access = format!(
        "{}.{}.sig",
        encode(r#"{"alg":"RS256","typ":"JWT"}"#),
        encode(r#"{"sub":"CHARACTER:EVE:111","name":"Director","scp":[]}"#),
      );
      let body = format!(r#"{{"access_token":"{access}","expires_in":1200,"refresh_token":"rotated-rt"}}"#);
      Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body.into_bytes(), "application/json"))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      infra::upsert(
        &db,
        2000,
        OwnerType::Corporation,
        "old-access",
        "old-rt",
        0,
        Some(111),
        None,
      )
      .await
      .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let sso = eve_sso::Client::new(http, "test-client").with_token_url(format!("{}/token", server.uri()));

      fresh_token(&db, &sso, 2000, OwnerType::Corporation).await.unwrap();

      let stored = infra::get(&db, 2000, OwnerType::Corporation).await.unwrap().unwrap();
      assert_eq!(
        stored.authorized_by(),
        Some(111),
        "a token refresh must not wipe the corporation's authorizing character"
      );
    }

    #[tokio::test]
    async fn it_refreshes_and_persists_a_near_expiry_token() {
      use base64::Engine as _;
      use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
      };

      let server = MockServer::start().await;
      let encode = |raw: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw);
      let access = format!(
        "{}.{}.sig",
        encode(r#"{"alg":"RS256","typ":"JWT"}"#),
        encode(r#"{"sub":"CHARACTER:EVE:77","name":"Pilot","scp":[]}"#),
      );
      let body = format!(r#"{{"access_token":"{access}","expires_in":1200,"refresh_token":"rotated-rt"}}"#);
      Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body.into_bytes(), "application/json"))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      infra::upsert(&db, 77, OwnerType::Character, "old-access", "old-rt", 0, None, None)
        .await
        .unwrap();
      let http = http::Client::builder(http::Cache::new(db.clone())).build();
      let sso = eve_sso::Client::new(http, "test-client").with_token_url(format!("{}/token", server.uri()));

      let grant = fresh_token(&db, &sso, 77, OwnerType::Character).await.unwrap().unwrap();

      assert_eq!(*grant.access_token(), access);
      let stored = infra::get(&db, 77, OwnerType::Character).await.unwrap().unwrap();
      assert_eq!(stored.refresh_token(), "rotated-rt");
    }

    #[tokio::test]
    async fn it_returns_none_when_no_credential_exists() {
      let db = store::open_test().await.unwrap();
      let sso = make_sso().await;

      assert!(
        fresh_token(&db, &sso, 999, OwnerType::Character)
          .await
          .unwrap()
          .is_none()
      );
    }

    #[tokio::test]
    async fn it_returns_the_stored_token_when_not_near_expiry() {
      let db = store::open_test().await.unwrap();
      let sso = make_sso().await;
      let far_future = Utc::now().timestamp() + 86_400;
      infra::upsert(
        &db,
        42,
        OwnerType::Character,
        "stored-token",
        "rt",
        far_future,
        None,
        None,
      )
      .await
      .unwrap();

      let grant = fresh_token(&db, &sso, 42, OwnerType::Character).await.unwrap().unwrap();

      assert_eq!(grant.access_token(), "stored-token");
    }
  }

  mod needs_refresh {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_false_when_well_before_expiry() {
      assert_eq!(needs_refresh(1_000, 0, 60), false);
    }

    #[test]
    fn it_is_true_once_expired() {
      assert_eq!(needs_refresh(1_000, 2_000, 60), true);
    }

    #[test]
    fn it_is_true_within_the_skew_window() {
      assert_eq!(needs_refresh(1_000, 950, 60), true);
    }
  }

  mod token_cache {
    use base64::Engine as _;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;

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

    async fn mount_token(server: &MockServer, character_id: i64, refreshes: u64) {
      let access = jwt_access(character_id);
      let body = format!(r#"{{"access_token":"{access}","expires_in":1200,"refresh_token":"rotated-rt"}}"#);
      Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body.into_bytes(), "application/json"))
        .expect(refreshes)
        .mount(server)
        .await;
    }

    mod get {
      use super::*;

      #[tokio::test]
      async fn it_memoizes_a_missing_credential_without_re_querying() {
        let db = store::open_test().await.unwrap();
        let sso = make_sso().await;

        let mut cache = TokenCache::new();
        assert!(
          cache
            .get(&db, &sso, 12_345, OwnerType::Character)
            .await
            .unwrap()
            .is_none()
        );
        assert!(
          cache
            .get(&db, &sso, 12_345, OwnerType::Character)
            .await
            .unwrap()
            .is_none()
        );
      }

      #[tokio::test]
      async fn it_refreshes_a_near_expiry_token_only_once_across_many_lookups_for_one_character() {
        let server = MockServer::start().await;
        mount_token(&server, 77, 1).await;
        let db = store::open_test().await.unwrap();
        infra::upsert(&db, 77, OwnerType::Character, "old-access", "old-rt", 0, None, None)
          .await
          .unwrap();
        let http = http::Client::builder(http::Cache::new(db.clone())).build();
        let sso = eve_sso::Client::new(http, "test-client").with_token_url(format!("{}/token", server.uri()));

        let mut cache = TokenCache::new();
        let first = cache.get(&db, &sso, 77, OwnerType::Character).await.unwrap().unwrap();
        let second = cache.get(&db, &sso, 77, OwnerType::Character).await.unwrap().unwrap();
        let third = cache.get(&db, &sso, 77, OwnerType::Character).await.unwrap().unwrap();

        assert_eq!(first.access_token(), second.access_token());
        assert_eq!(second.access_token(), third.access_token());
        assert_eq!(*first.access_token(), jwt_access(77));
      }

      #[tokio::test]
      async fn it_refreshes_each_unique_character_once() {
        let server = MockServer::start().await;
        mount_token(&server, 88, 1).await;
        let server2 = MockServer::start().await;
        mount_token(&server2, 99, 1).await;
        let db = store::open_test().await.unwrap();
        infra::upsert(&db, 88, OwnerType::Character, "a", "rt-a", 0, None, None)
          .await
          .unwrap();
        infra::upsert(&db, 99, OwnerType::Character, "b", "rt-b", 0, None, None)
          .await
          .unwrap();
        let http = http::Client::builder(http::Cache::new(db.clone())).build();
        let sso_a = eve_sso::Client::new(http.clone(), "test-client").with_token_url(format!("{}/token", server.uri()));
        let sso_b = eve_sso::Client::new(http, "test-client").with_token_url(format!("{}/token", server2.uri()));

        let mut cache = TokenCache::new();
        cache.get(&db, &sso_a, 88, OwnerType::Character).await.unwrap();
        cache.get(&db, &sso_b, 99, OwnerType::Character).await.unwrap();
        cache.get(&db, &sso_a, 88, OwnerType::Character).await.unwrap();
        cache.get(&db, &sso_b, 99, OwnerType::Character).await.unwrap();

        let grant_88 = cache.get(&db, &sso_a, 88, OwnerType::Character).await.unwrap().unwrap();
        let grant_99 = cache.get(&db, &sso_b, 99, OwnerType::Character).await.unwrap().unwrap();
        assert_eq!(*grant_88.access_token(), jwt_access(88));
        assert_eq!(*grant_99.access_token(), jwt_access(99));
      }

      #[tokio::test]
      async fn it_reuses_an_unexpired_token_without_any_sso_round_trip() {
        let server = MockServer::start().await;
        mount_token(&server, 55, 0).await;
        let db = store::open_test().await.unwrap();
        let far_future = Utc::now().timestamp() + 86_400;
        infra::upsert(
          &db,
          55,
          OwnerType::Character,
          "stored-token",
          "rt",
          far_future,
          None,
          None,
        )
        .await
        .unwrap();
        let http = http::Client::builder(http::Cache::new(db.clone())).build();
        let sso = eve_sso::Client::new(http, "test-client").with_token_url(format!("{}/token", server.uri()));

        let mut cache = TokenCache::new();
        let first = cache.get(&db, &sso, 55, OwnerType::Character).await.unwrap().unwrap();
        let second = cache.get(&db, &sso, 55, OwnerType::Character).await.unwrap().unwrap();

        assert_eq!(first.access_token(), "stored-token");
        assert_eq!(second.access_token(), "stored-token");
      }
    }
  }
}
