//! Client for EVE loyalty store ESI endpoints.

use crate::{Error, models::loyalty::LoyaltyOffer};

/// Client for a specific NPC corporation loyalty store.
pub struct Client<'a> {
  corp_id: i64,
  esi: &'a crate::Client,
}

impl<'a> Client<'a> {
  /// Creates a new `Client` for the NPC corporation with the given `corp_id`.
  pub(crate) fn new(esi: &'a crate::Client, corp_id: i64) -> Self {
    Self {
      corp_id,
      esi,
    }
  }

  /// Returns the loyalty store offers for this NPC corporation.
  pub async fn offers(&self) -> Result<Vec<LoyaltyOffer>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/loyalty/stores/{}/offers/", self.corp_id))
          .build(),
        None,
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

  mod offers {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_loyalty_offers() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/loyalty/stores/1000001/offers/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([
              {
                "ak_cost": null,
                "isk_cost": 0,
                "lp_cost": 100,
                "offer_id": 4001,
                "quantity": 1,
                "required_items": [
                  {"quantity": 10, "type_id": 34}
                ],
                "type_id": 12005
              }
            ])),
        )
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let offers = esi.loyalty(1_000_001).offers().await.unwrap();

      assert_eq!(offers.len(), 1);
      assert_eq!(offers[0].offer_id, 4001);
      assert_eq!(offers[0].lp_cost, 100);
      assert_eq!(offers[0].isk_cost, 0);
      assert_eq!(offers[0].type_id, 12005);
      assert!(offers[0].ak_cost.is_none());
      assert_eq!(offers[0].required_items.len(), 1);
      assert_eq!(offers[0].required_items[0].quantity, 10);
      assert_eq!(offers[0].required_items[0].type_id, 34);
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/loyalty/stores/9999999/offers/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"error": "Corporation not found"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.loyalty(9_999_999).offers().await;

      assert!(result.is_err());
    }
  }
}
