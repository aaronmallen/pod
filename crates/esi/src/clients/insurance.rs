//! Client for EVE insurance ESI endpoints.

use crate::{Error, models::insurance::InsurancePrice};

/// Client for insurance ESI endpoints.
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

  /// Returns insurance prices for all ship types.
  pub async fn prices(&self) -> Result<Vec<InsurancePrice>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/insurance/prices/").build(), None)
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

  mod prices {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_insurance_prices() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/insurance/prices/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([
              {
                "type_id": 582,
                "levels": [
                  {"cost": 1000.0, "name": "Basic", "payout": 5000.0},
                  {"cost": 2000.0, "name": "Standard", "payout": 10000.0}
                ]
              }
            ])),
        )
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let prices = esi.insurance().prices().await.unwrap();

      assert_eq!(prices.len(), 1);
      assert_eq!(prices[0].type_id, 582);
      assert_eq!(prices[0].levels.len(), 2);
      assert_eq!(prices[0].levels[0].name, "Basic");
      assert_eq!(prices[0].levels[1].payout, 10_000.0);
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/insurance/prices/"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({"error": "Internal server error"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.insurance().prices().await;

      assert!(result.is_err());
    }
  }
}
