//! Character wallet and market order endpoints.

use crate::{
  Error,
  clients::character::AuthenticatedClient,
  models::character::{CharacterOrder, CharacterWalletBalance, WalletJournalEntry, WalletTransaction},
};

impl AuthenticatedClient<'_> {
  /// Returns all active market orders for this character.
  pub async fn orders(&self) -> Result<Vec<CharacterOrder>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/orders/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns historical market orders for this character (paginated).
  pub async fn order_history(&self) -> Result<Vec<CharacterOrder>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/orders/history/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the ISK balance of this character's wallet.
  pub async fn wallet_balance(&self) -> Result<CharacterWalletBalance, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/wallet/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the wallet journal for this character (paginated).
  pub async fn wallet_journal(&self) -> Result<Vec<WalletJournalEntry>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v6/characters/{}/wallet/journal/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the wallet transactions for this character (paginated).
  pub async fn wallet_transactions(&self) -> Result<Vec<WalletTransaction>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/wallet/transactions/", self.id))
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

  mod wallet_balance {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_wallet_balance_on_success() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/characters/90000001/wallet/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"5000000.0"#, "application/json"))
        .mount(&server)
        .await;
      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.wallet_balance().await.unwrap();

      assert_eq!(result.0, 5000000.0);
    }

    #[tokio::test]
    async fn it_returns_api_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/characters/90000001/wallet/"))
        .respond_with(ResponseTemplate::new(401).set_body_raw(r#"{"error": "Unauthorized"}"#, "application/json"))
        .mount(&server)
        .await;
      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.wallet_balance().await;

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
