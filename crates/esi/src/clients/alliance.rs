//! Client for EVE alliance ESI endpoints.

use crate::{
  Error,
  models::{
    alliance::{AllianceContact, AllianceContactLabel, AllianceDetail, AllianceIcons},
    auth::Grant,
  },
};

/// Client for alliance-scoped ESI endpoints.
pub struct Client<'a> {
  esi: &'a crate::Client,
  id: i64,
}

impl<'a> Client<'a> {
  /// Creates a new `Client` for the alliance with the given `id`.
  pub(crate) fn new(esi: &'a crate::Client, id: i64) -> Self {
    Self {
      esi,
      id,
    }
  }

  /// Returns an authenticated alliance client bound to the given grant.
  pub fn auth(&self, grant: &'a Grant) -> AuthenticatedClient<'a> {
    AuthenticatedClient {
      esi: self.esi,
      grant,
      id: self.id,
    }
  }

  /// Returns the IDs of corporations in this alliance.
  pub async fn corporation_ids(&self) -> Result<Vec<i64>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/alliances/{}/corporations/", self.id))
          .build(),
        None,
      )
      .await
  }

  /// Returns public information for this alliance.
  pub async fn detail(&self) -> Result<AllianceDetail, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v3/alliances/{}/", self.id))
          .build(),
        None,
      )
      .await
  }

  /// Returns the icon URLs for this alliance.
  pub async fn icons(&self) -> Result<AllianceIcons, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/alliances/{}/icons/", self.id))
          .build(),
        None,
      )
      .await
  }
}

/// Authenticated client for alliance-scoped ESI endpoints requiring a valid OAuth2 grant.
pub struct AuthenticatedClient<'a> {
  esi: &'a crate::Client,
  grant: &'a Grant,
  id: i64,
}

impl<'a> AuthenticatedClient<'a> {
  /// Returns the alliance contact labels.
  pub async fn contact_labels(&self) -> Result<Vec<AllianceContactLabel>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/alliances/{}/contacts/labels/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the alliance contact list (paginated).
  pub async fn contacts(&self) -> Result<Vec<AllianceContact>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/alliances/{}/contacts/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}

#[cfg(test)]
mod tests {
  use std::time::{Duration, SystemTime};

  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  fn make_esi(server_uri: &str) -> crate::Client {
    crate::Client::builder("test-client")
      .base_url(server_uri)
      .build()
      .unwrap()
  }

  fn make_grant() -> crate::models::auth::Grant {
    crate::models::auth::Grant::new(
      "test-token",
      90_000_001i64,
      "Test Char",
      SystemTime::now() + Duration::from_secs(3600),
      "refresh",
      vec![],
    )
  }

  mod detail {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_alliance_detail() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v3/alliances/99000006/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "creator_corporation_id": 98000001i64,
          "creator_id": 90000001i64,
          "date_founded": "2010-01-01T00:00:00Z",
          "executor_corporation_id": 98000002i64,
          "name": "Test Alliance",
          "ticker": "TSTL"
        })))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.alliance(99_000_006i64).detail().await.unwrap();

      assert_eq!(result.name, "Test Alliance");
      assert_eq!(result.ticker, "TSTL");
      assert_eq!(result.creator_id, 90_000_001i64);
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v3/alliances/99000006/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"error": "Alliance not found"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.alliance(99_000_006i64).detail().await;

      assert!(result.is_err());
    }
  }

  mod contacts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_alliance_contacts() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v2/alliances/99000006/contacts/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([
              {"contact_id": 90000002i64, "contact_type": "character", "standing": 5.0}
            ])),
        )
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let grant = make_grant();
      let result = esi.alliance(99_000_006i64).auth(&grant).contacts().await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].contact_id, 90_000_002i64);
      assert_eq!(result[0].standing, 5.0);
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v2/alliances/99000006/contacts/"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({"error": "Forbidden"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let grant = make_grant();
      let result = esi.alliance(99_000_006i64).auth(&grant).contacts().await;

      assert!(result.is_err());
    }
  }
}
