//! Client for EVE corporation ESI endpoints.

pub mod assets;
pub mod contacts;
pub mod contracts;
pub mod industry;
pub mod members;
pub mod misc;
pub mod wallets;

use crate::{
  Client as EsiClient, Error,
  models::{
    auth::Grant,
    corporation::{AllianceHistoryEntry, CorporationDetail, CorporationIcons},
  },
};

/// Client for corporation ESI endpoints.
pub struct Client<'a> {
  pub(in crate::clients::corporation) esi: &'a EsiClient,
  pub(in crate::clients::corporation) id: i64,
}

impl<'a> Client<'a> {
  /// Creates a new `Client` for the corporation with the given `id`.
  pub(crate) fn new(esi: &'a EsiClient, id: i64) -> Self {
    Self {
      esi,
      id,
    }
  }

  /// Returns an authenticated client bound to the given grant.
  pub fn auth<'b>(&'b self, grant: &'b Grant) -> AuthenticatedClient<'b> {
    AuthenticatedClient {
      esi: self.esi,
      grant,
      id: self.id,
    }
  }

  /// Returns the alliance history for this corporation.
  pub async fn alliance_history(&self) -> Result<Vec<AllianceHistoryEntry>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v3/corporations/{}/alliancehistory/", self.id))
          .build(),
        None,
      )
      .await
  }

  /// Returns public information for this corporation.
  pub async fn detail(&self) -> Result<CorporationDetail, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v5/corporations/{}/", self.id))
          .build(),
        None,
      )
      .await
  }

  /// Returns the icon URLs for this corporation.
  pub async fn icons(&self) -> Result<CorporationIcons, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/icons/", self.id))
          .build(),
        None,
      )
      .await
  }
}

/// Authenticated client for corporation ESI endpoints requiring a valid OAuth2 grant.
pub struct AuthenticatedClient<'a> {
  pub(in crate::clients::corporation) esi: &'a EsiClient,
  pub(in crate::clients::corporation) grant: &'a Grant,
  pub(in crate::clients::corporation) id: i64,
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  mod detail {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_corporation_detail() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v5/corporations/109299958/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "alliance_id": null,
          "ceo_id": 180548812i64,
          "creator_id": 180548812i64,
          "date_founded": null,
          "description": null,
          "faction_id": null,
          "home_station_id": null,
          "member_count": 10,
          "name": "Test Corp",
          "shares": null,
          "tax_rate": 0.1,
          "ticker": "TEST",
          "url": null,
          "war_eligible": null
        })))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let corp = esi.corporation(109_299_958i64);

      let detail = corp.detail().await.unwrap();

      assert_eq!(detail.name, "Test Corp");
      assert_eq!(detail.ticker, "TEST");
      assert_eq!(detail.member_count, 10);
    }

    #[tokio::test]
    async fn it_returns_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v5/corporations/109299958/"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({"error": "Unauthorized"})))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let corp = esi.corporation(109_299_958i64);

      let result = corp.detail().await;

      assert!(result.is_err());
    }
  }

  mod alliance_history {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_alliance_history() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v3/corporations/109299958/alliancehistory/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          {
            "alliance_id": 99_000_006i64,
            "is_deleted": null,
            "record_id": 1,
            "start_date": "2010-01-01T00:00:00Z"
          }
        ])))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let corp = esi.corporation(109_299_958i64);

      let history = corp.alliance_history().await.unwrap();

      assert_eq!(history.len(), 1);
      assert_eq!(history[0].record_id, 1);
      assert_eq!(history[0].start_date, "2010-01-01T00:00:00Z");
    }

    #[tokio::test]
    async fn it_returns_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v3/corporations/109299958/alliancehistory/"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({"error": "Unauthorized"})))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let corp = esi.corporation(109_299_958i64);

      let result = corp.alliance_history().await;

      assert!(result.is_err());
    }
  }
}
