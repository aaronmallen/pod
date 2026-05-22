//! Client for EVE industry ESI endpoints.

use crate::{
  Error,
  models::industry::{IndustryFacility, IndustrySolarSystem},
};

/// Client for industry ESI endpoints.
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

  /// Returns all industry facilities.
  pub async fn facilities(&self) -> Result<Vec<IndustryFacility>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/industry/facilities/").build(), None)
      .await
  }

  /// Returns solar system industry cost indices.
  pub async fn systems(&self) -> Result<Vec<IndustrySolarSystem>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/industry/systems/").build(), None)
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

  mod facilities {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_industry_facilities() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/industry/facilities/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([
              {
                "facility_id": 60012526i64,
                "owner_id": 1000001i64,
                "region_id": 10000002i64,
                "solar_system_id": 30000142i64,
                "solar_system_security": 0.9,
                "tax": 0.1,
                "type_id": 1657
              }
            ])),
        )
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let facilities = esi.industry().facilities().await.unwrap();

      assert_eq!(facilities.len(), 1);
      assert_eq!(facilities[0].facility_id, 60_012_526i64);
      assert_eq!(facilities[0].type_id, 1657);
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/industry/facilities/"))
        .respond_with(ResponseTemplate::new(503).set_body_json(serde_json::json!({"error": "Service unavailable"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.industry().facilities().await;

      assert!(result.is_err());
    }
  }
}
