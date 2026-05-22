//! Client for EVE in-game UI ESI endpoints.

use serde::Serialize;

use crate::{Client as EsiClient, Error, models::auth::Grant};

/// Body for the new mail window endpoint.
#[derive(Serialize)]
pub struct NewMailBody {
  pub body: String,
  pub recipients: Vec<i64>,
  pub subject: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub to_corp_or_alliance_id: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub to_mailing_list_id: Option<i64>,
}

/// Client for in-game UI ESI endpoints.
pub struct Client<'a> {
  esi: &'a EsiClient,
}

impl<'a> Client<'a> {
  /// Creates a new `Client` for UI endpoints.
  pub(crate) fn new(esi: &'a EsiClient) -> Self {
    Self {
      esi,
    }
  }

  /// Returns an authenticated client bound to the given grant.
  pub fn auth<'b>(&'b self, grant: &'b Grant) -> AuthenticatedClient<'b> {
    AuthenticatedClient {
      esi: self.esi,
      grant,
    }
  }
}

/// Authenticated client for in-game UI ESI endpoints requiring a valid OAuth2 grant.
pub struct AuthenticatedClient<'a> {
  esi: &'a EsiClient,
  grant: &'a Grant,
}

impl AuthenticatedClient<'_> {
  /// Adds a waypoint to the autopilot route.
  pub async fn add_waypoint(&self, destination_id: i64) -> Result<(), Error> {
    self
      .esi
      .http()
      .post_empty(
        &self
          .esi
          .url_builder()
          .path("v2/ui/autopilot/waypoint/")
          .param("add_to_beginning", "false")
          .param("clear_other_waypoints", "false")
          .param("destination_id", destination_id.to_string())
          .build(),
        &(),
        self.grant.access_token(),
      )
      .await
  }

  /// Opens the in-game contract window for the given contract.
  pub async fn open_contract(&self, contract_id: i64) -> Result<(), Error> {
    self
      .esi
      .http()
      .post_empty(
        &self
          .esi
          .url_builder()
          .path("v1/ui/openwindow/contract/")
          .param("contract_id", contract_id.to_string())
          .build(),
        &(),
        self.grant.access_token(),
      )
      .await
  }

  /// Opens the in-game information window for the given entity.
  pub async fn open_information(&self, target_id: i64) -> Result<(), Error> {
    self
      .esi
      .http()
      .post_empty(
        &self
          .esi
          .url_builder()
          .path("v1/ui/openwindow/information/")
          .param("target_id", target_id.to_string())
          .build(),
        &(),
        self.grant.access_token(),
      )
      .await
  }

  /// Opens the in-game market details window for the given type.
  pub async fn open_market_details(&self, type_id: i32) -> Result<(), Error> {
    self
      .esi
      .http()
      .post_empty(
        &self
          .esi
          .url_builder()
          .path("v1/ui/openwindow/marketdetails/")
          .param("type_id", type_id.to_string())
          .build(),
        &(),
        self.grant.access_token(),
      )
      .await
  }

  /// Opens the in-game new mail compose window.
  pub async fn open_new_mail(&self, body: &NewMailBody) -> Result<(), Error> {
    self
      .esi
      .http()
      .post_empty(
        &self.esi.url_builder().path("v1/ui/openwindow/newmail/").build(),
        body,
        self.grant.access_token(),
      )
      .await
  }
}

#[cfg(test)]
mod tests {
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
      std::time::SystemTime::now() + std::time::Duration::from_secs(3600),
      "refresh",
      vec![],
    )
  }

  mod add_waypoint {
    use super::*;

    #[tokio::test]
    async fn it_posts_to_autopilot_waypoint_on_success() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/v2/ui/autopilot/waypoint/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let grant = make_grant();
      let result = esi.ui().auth(&grant).add_waypoint(30_000_142).await;

      assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/v2/ui/autopilot/waypoint/"))
        .respond_with(ResponseTemplate::new(422).set_body_json(serde_json::json!({"error": "Unprocessable Entity"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let grant = make_grant();
      let result = esi.ui().auth(&grant).add_waypoint(30_000_142).await;

      assert!(result.is_err());
    }
  }
}
