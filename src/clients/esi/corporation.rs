use crate::clients::{
  self,
  esi::{
    Client as EsiClient,
    models::corporation::{
      CorporationAsset, CorporationDivisions, CorporationInfo, CorporationWalletBalance, CorporationWalletJournalEntry,
      CorporationWalletTransaction, MemberRole,
    },
  },
  eve_sso::Grant,
};

pub struct AuthenticatedClient<'a> {
  esi: &'a EsiClient,
  grant: &'a Grant,
}

impl<'a> AuthenticatedClient<'a> {
  pub fn new(esi: &'a EsiClient, grant: &'a Grant) -> Self {
    Self {
      esi,
      grant,
    }
  }

  pub async fn assets(&self, corporation_id: i64) -> Result<Vec<CorporationAsset>, clients::Error> {
    let url = self.esi.url(&format!("v5/corporations/{corporation_id}/assets/"));
    self
      .esi
      .http()
      .get_json_paginated(&url, Some(self.grant.access_token()))
      .await
  }

  pub async fn divisions(&self, corporation_id: i64) -> Result<CorporationDivisions, clients::Error> {
    let url = self.esi.url(&format!("v1/corporations/{corporation_id}/divisions/"));
    self.esi.http().get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn member_roles(&self, corporation_id: i64) -> Result<Vec<MemberRole>, clients::Error> {
    let url = self.esi.url(&format!("v2/corporations/{corporation_id}/roles/"));
    self.esi.http().get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn wallet_journal(
    &self,
    corporation_id: i64,
    division: i32,
  ) -> Result<Vec<CorporationWalletJournalEntry>, clients::Error> {
    let url = self
      .esi
      .url(&format!("v4/corporations/{corporation_id}/wallets/{division}/journal/"));
    self
      .esi
      .http()
      .get_json_paginated(&url, Some(self.grant.access_token()))
      .await
  }

  pub async fn wallet_transactions(
    &self,
    corporation_id: i64,
    division: i32,
  ) -> Result<Vec<CorporationWalletTransaction>, clients::Error> {
    let url = self.esi.url(&format!(
      "v1/corporations/{corporation_id}/wallets/{division}/transactions/"
    ));
    self.esi.http().get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn wallets(&self, corporation_id: i64) -> Result<Vec<CorporationWalletBalance>, clients::Error> {
    let url = self.esi.url(&format!("v1/corporations/{corporation_id}/wallets/"));
    self.esi.http().get_json(&url, Some(self.grant.access_token())).await
  }
}

pub struct PublicClient<'a> {
  esi: &'a EsiClient,
}

impl<'a> PublicClient<'a> {
  pub fn new(esi: &'a EsiClient) -> Self {
    Self {
      esi,
    }
  }

  pub async fn info(&self, corporation_id: i64) -> Result<CorporationInfo, clients::Error> {
    let url = self.esi.url(&format!("v5/corporations/{corporation_id}/"));
    self.esi.http().get_json(&url, None).await
  }
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
  };

  use super::*;
  use crate::{clients::http, store};

  async fn make_esi(base_url: &str) -> EsiClient {
    let db = store::open_test().await.unwrap();
    let cache = http::Cache::new(db);
    let http = http::Client::builder(cache).build();
    EsiClient::with_base_url(http, base_url)
  }

  mod authenticated_client {
    use super::*;

    mod assets {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_assets() {
        let server = MockServer::start().await;
        let body = r#"[{"is_singleton":true,"item_id":1,"location_flag":"CorpDeliveries","location_id":60,"location_type":"station","quantity":3,"type_id":34}]"#;
        Mock::given(method("GET"))
          .and(path("/v5/corporations/2000/assets/"))
          .and(header("Authorization", "Bearer corp-token"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "1")
              .set_body_raw(body, "application/json"),
          )
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let assets = esi.corporation_authenticated(&grant).assets(2000).await.unwrap();

        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].quantity, 3);
      }
    }

    mod divisions {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_divisions() {
        let server = MockServer::start().await;
        let body = r#"{
          "hangar": [{"division": 1, "name": "Hangar One"}, {"division": 2}],
          "wallet": [{"division": 1, "name": "Master Wallet"}, {"division": 2, "name": "Second Wallet"}]
        }"#;
        Mock::given(method("GET"))
          .and(path("/v1/corporations/2000/divisions/"))
          .and(header("Authorization", "Bearer corp-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let divisions = esi.corporation_authenticated(&grant).divisions(2000).await.unwrap();

        assert_eq!(divisions.hangar.len(), 2);
        assert_eq!(divisions.hangar[0].division, 1);
        assert_eq!(divisions.hangar[0].name.as_deref(), Some("Hangar One"));
        assert_eq!(divisions.hangar[1].name, None);
        assert_eq!(divisions.wallet.len(), 2);
        assert_eq!(divisions.wallet[0].name.as_deref(), Some("Master Wallet"));
      }
    }

    mod member_roles {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_returns_member_roles() {
        let server = MockServer::start().await;
        let body = r#"[{"character_id":123,"roles":["Director","Accountant"]}]"#;
        Mock::given(method("GET"))
          .and(path("/v2/corporations/2000/roles/"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let roles = esi.corporation_authenticated(&grant).member_roles(2000).await.unwrap();

        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].character_id, 123);
        assert_eq!(roles[0].roles, vec!["Director", "Accountant"]);
      }
    }

    mod wallet_journal {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_honors_pagination() {
        let server = MockServer::start().await;
        let page_one = r#"[{"id":1,"date":"2026-01-01T00:00:00Z","description":"Bounty","ref_type":"bounty_prizes","amount":100.0,"balance":1100.0,"first_party_id":7,"second_party_id":8}]"#;
        let page_two =
          r#"[{"id":2,"date":"2026-01-02T00:00:00Z","description":"Tax","ref_type":"corporation_account_withdrawal"}]"#;
        Mock::given(method("GET"))
          .and(path("/v4/corporations/2000/wallets/1/journal/"))
          .and(header("Authorization", "Bearer corp-token"))
          .and(wiremock::matchers::query_param("page", "1"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "2")
              .set_body_raw(page_one, "application/json"),
          )
          .mount(&server)
          .await;
        Mock::given(method("GET"))
          .and(path("/v4/corporations/2000/wallets/1/journal/"))
          .and(header("Authorization", "Bearer corp-token"))
          .and(wiremock::matchers::query_param("page", "2"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "2")
              .set_body_raw(page_two, "application/json"),
          )
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let entries = esi
          .corporation_authenticated(&grant)
          .wallet_journal(2000, 1)
          .await
          .unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, 1);
        assert_eq!(entries[0].amount, Some(100.0));
        assert_eq!(entries[0].ref_type, "bounty_prizes");
        assert_eq!(entries[1].id, 2);
        assert_eq!(entries[1].amount, None);
      }
    }

    mod wallet_transactions {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_transactions() {
        let server = MockServer::start().await;
        let body = r#"[{"client_id":1001,"date":"2026-01-01T00:00:00Z","is_buy":true,"journal_ref_id":555,"location_id":60003760,"quantity":10,"transaction_id":9001,"type_id":34,"unit_price":5.5}]"#;
        Mock::given(method("GET"))
          .and(path("/v1/corporations/2000/wallets/1/transactions/"))
          .and(header("Authorization", "Bearer corp-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let transactions = esi
          .corporation_authenticated(&grant)
          .wallet_transactions(2000, 1)
          .await
          .unwrap();

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].transaction_id, 9001);
        assert!(transactions[0].is_buy);
        assert_eq!(transactions[0].journal_ref_id, 555);
        assert_eq!(transactions[0].unit_price, 5.5);
      }
    }

    mod wallets {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_wallet_balances() {
        let server = MockServer::start().await;
        let body = r#"[{"division":1,"balance":1234.56},{"division":2,"balance":0.0}]"#;
        Mock::given(method("GET"))
          .and(path("/v1/corporations/2000/wallets/"))
          .and(header("Authorization", "Bearer corp-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let wallets = esi.corporation_authenticated(&grant).wallets(2000).await.unwrap();

        assert_eq!(wallets.len(), 2);
        assert_eq!(wallets[0].division, 1);
        assert_eq!(wallets[0].balance, 1234.56);
      }
    }
  }

  mod public_client {
    use super::*;

    mod info {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_returns_corporation_info() {
        let server = MockServer::start().await;
        let body = r#"{
          "ceo_id": 180548812,
          "creator_id": 180548812,
          "member_count": 656,
          "name": "C C P",
          "tax_rate": 0.0,
          "ticker": "-CCP-"
        }"#;
        Mock::given(method("GET"))
          .and(path("/v5/corporations/2000/"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;

        let info = esi.corporation().info(2000).await.unwrap();

        assert_eq!(info.ceo_id, 180548812);
        assert_eq!(info.name, "C C P");
        assert_eq!(info.ticker, "-CCP-");
      }

      #[tokio::test]
      async fn it_returns_http_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/v5/corporations/2000/"))
          .respond_with(ResponseTemplate::new(404))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;

        let result = esi.corporation().info(2000).await;

        assert!(matches!(result, Err(clients::Error::Http(_))));
      }
    }
  }
}
