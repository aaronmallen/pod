//! Corporation wallet and market order endpoints.

use crate::{
  Error,
  clients::corporation::AuthenticatedClient,
  models::corporation::{
    CorporationOrder, CorporationWallet, CorporationWalletJournalEntry, CorporationWalletTransaction,
  },
};

impl AuthenticatedClient<'_> {
  /// Returns active market orders for this corporation (paginated).
  pub async fn orders(&self) -> Result<Vec<CorporationOrder>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v3/corporations/{}/orders/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns historical market orders for this corporation (paginated).
  pub async fn order_history(&self) -> Result<Vec<CorporationOrder>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/orders/history/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the wallet divisions and balances for this corporation.
  pub async fn wallets(&self) -> Result<Vec<CorporationWallet>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/wallets/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the wallet journal for a division of this corporation (paginated).
  pub async fn wallet_journal(&self, division: i32) -> Result<Vec<CorporationWalletJournalEntry>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v4/corporations/{}/wallets/{division}/journal/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the wallet transactions for a division of this corporation (paginated).
  pub async fn wallet_transactions(&self, division: i32) -> Result<Vec<CorporationWalletTransaction>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/wallets/{division}/transactions/", self.id))
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

  mod wallets {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_wallet_divisions_on_success() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/corporations/109299958/wallets/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          {"balance": 1_500_000.0, "division": 1},
          {"balance": 250_000.0, "division": 2}
        ])))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let corp = esi.corporation(109_299_958i64);
      let grant = make_grant();
      let auth = corp.auth(&grant);

      let result = auth.wallets().await.unwrap();

      assert_eq!(result.len(), 2);
      assert_eq!(result[0].division, 1);
      assert_eq!(result[0].balance, 1_500_000.0f64);
      assert_eq!(result[1].division, 2);
    }

    #[tokio::test]
    async fn it_returns_api_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/corporations/109299958/wallets/"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({"error": "Unauthorized"})))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let corp = esi.corporation(109_299_958i64);
      let grant = make_grant();
      let auth = corp.auth(&grant);

      let result = auth.wallets().await;

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
