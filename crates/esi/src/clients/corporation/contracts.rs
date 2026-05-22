//! Corporation contract endpoints.

use crate::{
  Error,
  clients::corporation::AuthenticatedClient,
  models::corporation::{ContractBid, ContractItem, CorporationContract},
};

impl AuthenticatedClient<'_> {
  /// Returns all contracts for this corporation (paginated).
  pub async fn contracts(&self) -> Result<Vec<CorporationContract>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/contracts/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the bids on a contract.
  pub async fn contract_bids(&self, contract_id: i64) -> Result<Vec<ContractBid>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/contracts/{contract_id}/bids/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the items in a contract.
  pub async fn contract_items(&self, contract_id: i64) -> Result<Vec<ContractItem>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/contracts/{contract_id}/items/", self.id))
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

  fn make_grant() -> crate::models::auth::Grant {
    crate::models::auth::Grant::new(
      "test-token",
      123_456_789i64,
      "Test Member",
      SystemTime::now() + Duration::from_secs(3600),
      "refresh",
      vec![],
    )
  }

  mod contracts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_corporation_contracts() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/corporations/109299958/contracts/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([
              {
                "acceptor_id": 0i64,
                "assignee_id": 0i64,
                "availability": "public",
                "buyout": null,
                "collateral": null,
                "contract_id": 123_456i64,
                "date_accepted": null,
                "date_completed": null,
                "date_expired": "2025-12-31T00:00:00Z",
                "date_issued": "2025-01-01T00:00:00Z",
                "days_to_complete": null,
                "end_location_id": null,
                "for_corporation": true,
                "issuer_corporation_id": 109_299_958i64,
                "issuer_id": 123_456_789i64,
                "price": 1000000.0,
                "reward": null,
                "start_location_id": null,
                "status": "outstanding",
                "title": null,
                "type": "item_exchange",
                "volume": null
              }
            ])),
        )
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let corp = esi.corporation(109_299_958i64);
      let auth = corp.auth(&grant);

      let contracts = auth.contracts().await.unwrap();

      assert_eq!(contracts.len(), 1);
      assert_eq!(contracts[0].contract_id, 123_456i64);
      assert_eq!(contracts[0].status, "outstanding");
      assert_eq!(contracts[0].r#type, "item_exchange");
    }

    #[tokio::test]
    async fn it_returns_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/corporations/109299958/contracts/"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({"error": "Unauthorized"})))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let corp = esi.corporation(109_299_958i64);
      let auth = corp.auth(&grant);

      let result = auth.contracts().await;

      assert!(result.is_err());
    }
  }
}
