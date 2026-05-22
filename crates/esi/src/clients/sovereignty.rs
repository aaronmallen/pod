//! Client for EVE sovereignty ESI endpoints.

use crate::{
  Error,
  models::sovereignty::{SovereigntyCampaign, SovereigntyMap, SovereigntyStructure},
};

/// Client for sovereignty ESI endpoints.
pub struct Client<'a> {
  esi: &'a crate::Client,
}

impl<'a> Client<'a> {
  /// Creates a new `Client`.
  pub(crate) fn new(esi: &'a crate::Client) -> Self {
    Self {
      esi,
    }
  }

  /// Returns all active sovereignty campaigns.
  pub async fn campaigns(&self) -> Result<Vec<SovereigntyCampaign>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/sovereignty/campaigns/").build(), None)
      .await
  }

  /// Returns sovereignty data for all solar systems.
  pub async fn map(&self) -> Result<Vec<SovereigntyMap>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/sovereignty/map/").build(), None)
      .await
  }

  /// Returns all sovereignty structures.
  pub async fn structures(&self) -> Result<Vec<SovereigntyStructure>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/sovereignty/structures/").build(), None)
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

  mod campaigns {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_sovereignty_campaigns() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/sovereignty/campaigns/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([
              {
                "attackers_score": 0.4,
                "campaign_id": 32833,
                "constellation_id": 20000020,
                "defender_id": 1695357456,
                "defender_score": 0.6,
                "event_type": "ihub_defense",
                "participants": null,
                "solar_system_id": 30000020,
                "start_time": "2024-01-15T10:00:00Z",
                "structure_id": 1_021_374_423_645_i64
              }
            ])),
        )
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let campaigns = esi.sovereignty().campaigns().await.unwrap();

      assert_eq!(campaigns.len(), 1);
      assert_eq!(campaigns[0].campaign_id, 32833);
      assert_eq!(campaigns[0].event_type, "ihub_defense");
      assert_eq!(campaigns[0].solar_system_id, 30000020);
      assert_eq!(campaigns[0].defender_id, Some(1_695_357_456));
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/sovereignty/campaigns/"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({"error": "Internal server error"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.sovereignty().campaigns().await;

      assert!(result.is_err());
    }
  }
}
