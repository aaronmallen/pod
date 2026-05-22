//! Character contract endpoints.

use crate::{
  Error,
  clients::character::AuthenticatedClient,
  models::character::{CharacterContract, ContractBid, ContractItem},
};

impl AuthenticatedClient<'_> {
  /// Returns all contracts for this character (paginated).
  pub async fn contracts(&self) -> Result<Vec<CharacterContract>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/contracts/", self.id))
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
          .path(format!("v1/characters/{}/contracts/{contract_id}/bids/", self.id))
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
          .path(format!("v1/characters/{}/contracts/{contract_id}/items/", self.id))
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
      90_000_001i64,
      "Test Char",
      SystemTime::now() + Duration::from_secs(3600),
      "refresh",
      vec![],
    )
  }

  mod contracts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_contracts_for_character() {
      let server = MockServer::start().await;

      Mock::given(method("GET"))
        .and(path("/v1/characters/90000001/contracts/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([
              {
                "acceptor_id": 0,
                "assignee_id": 0,
                "availability": "personal",
                "contract_id": 1001,
                "date_expired": "2025-01-01T00:00:00Z",
                "date_issued": "2024-12-01T00:00:00Z",
                "for_corporation": false,
                "issuer_corporation_id": 109299958,
                "issuer_id": 90000001,
                "status": "outstanding",
                "type": "item_exchange"
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
      let auth = esi.character(&grant);

      let result = auth.contracts().await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].contract_id, 1001i64);
      assert_eq!(result[0].status, "outstanding");
      assert_eq!(result[0].r#type, "item_exchange");
    }

    #[tokio::test]
    async fn it_returns_error_on_401() {
      let server = MockServer::start().await;

      Mock::given(method("GET"))
        .and(path("/v1/characters/90000001/contracts/"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({"error": "Unauthorized"})))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.contracts().await;

      assert!(matches!(
        result,
        Err(crate::Error::Api {
          status: 401,
          ..
        })
      ));
    }
  }
}
