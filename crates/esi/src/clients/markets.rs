//! Client for EVE market order ESI endpoints (unauthenticated, region-scoped).

pub mod orders;

use crate::Client as EsiClient;

/// Client for market order ESI endpoints.
pub struct Client<'a> {
  pub(super) esi: &'a EsiClient,
}

impl<'a> Client<'a> {
  /// Creates a new `Client`.
  pub(crate) fn new(esi: &'a EsiClient) -> Self {
    Self {
      esi,
    }
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

  mod lowest_jita_sell {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_lowest_sell_price() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/markets/10000002/orders/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          {"is_buy_order": false, "price": 1000.0},
          {"is_buy_order": false, "price": 900.0}
        ])))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.markets().lowest_jita_sell(34i32).await.unwrap();

      assert_eq!(result, Some(900.0f64));
    }

    #[tokio::test]
    async fn it_returns_error_on_500() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/markets/10000002/orders/"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({"error": "Internal server error"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.markets().lowest_jita_sell(34i32).await;

      assert!(result.is_err());
    }
  }
}
