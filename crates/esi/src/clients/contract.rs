//! Client for EVE public contract ESI endpoints.

use crate::{
  Error,
  models::contract::{ContractBid, ContractItem, PublicContract},
};

/// Client for public contract ESI endpoints.
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

  /// Returns bids on an auction contract.
  pub async fn bids(&self, contract_id: i64) -> Result<Vec<ContractBid>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/contracts/public/bids/{contract_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns items in a public contract.
  pub async fn items(&self, contract_id: i64) -> Result<Vec<ContractItem>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/contracts/public/items/{contract_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns public contracts in the given region (paginated).
  pub async fn region(&self, region_id: i64) -> Result<Vec<PublicContract>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/contracts/public/{region_id}/"))
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

  mod bids {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_contract_bids() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/contracts/public/bids/12345/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          {
            "amount": 1500000.0,
            "bid_id": 1i64,
            "bidder_id": 90000001i64,
            "date_bid": "2023-01-01T12:00:00Z"
          }
        ])))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.contract().bids(12_345i64).await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].bid_id, 1i64);
      assert_eq!(result[0].amount, 1_500_000.0);
      assert_eq!(result[0].bidder_id, 90_000_001i64);
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/contracts/public/bids/12345/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"error": "Contract not found"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.contract().bids(12_345i64).await;

      assert!(result.is_err());
    }
  }

  mod region {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_region_contracts() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/contracts/public/10000002/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([
              {
                "contract_id": 99000001i64,
                "date_expired": "2023-12-31T00:00:00Z",
                "date_issued": "2023-01-01T00:00:00Z",
                "issuer_corporation_id": 98000001i64,
                "issuer_id": 90000001i64,
                "type": "item_exchange"
              }
            ])),
        )
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.contract().region(10_000_002i64).await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].contract_id, 99_000_001i64);
      assert_eq!(result[0].r#type, "item_exchange");
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/contracts/public/10000002/"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({"error": "Internal server error"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.contract().region(10_000_002i64).await;

      assert!(result.is_err());
    }
  }
}
