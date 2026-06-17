use crate::clients::{
  self,
  esi::{
    Client as EsiClient,
    models::{
      assets::AssetName,
      blueprint::Blueprint,
      character::{Contact, ContactLabel, Contract, ContractBid, ContractItem, RecentKillmail, Standing},
      corporation::{
        CorporationAsset, CorporationDivisions, CorporationInfo, CorporationStructure, CorporationWalletBalance,
        CorporationWalletJournalEntry, CorporationWalletTransaction, MemberRole,
      },
      industry::{IndustryJob, MiningExtraction},
    },
  },
  eve_sso::Grant,
};

#[allow(dead_code)]
const ASSET_NAMES_BATCH_SIZE: usize = 1000;

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
    let url = self.esi.url(&format!("corporations/{corporation_id}/assets/"));
    self.esi.get_json_paginated(&url, Some(self.grant.access_token())).await
  }

  #[allow(dead_code)]
  pub async fn assets_names(&self, corporation_id: i64, item_ids: &[i64]) -> Result<Vec<AssetName>, clients::Error> {
    let url = self.esi.url(&format!("corporations/{corporation_id}/assets/names/"));
    let mut names = Vec::new();
    for batch in item_ids.chunks(ASSET_NAMES_BATCH_SIZE) {
      let page: Vec<AssetName> = self.esi.post_json(&url, &batch, self.grant.access_token()).await?;
      names.extend(page);
    }
    Ok(names)
  }

  #[allow(dead_code)]
  pub async fn blueprints(&self, corporation_id: i64) -> Result<Vec<Blueprint>, clients::Error> {
    let url = self.esi.url(&format!("corporations/{corporation_id}/blueprints/"));
    self.esi.get_json_paginated(&url, Some(self.grant.access_token())).await
  }

  pub async fn contact_labels(&self, corporation_id: i64) -> Result<Vec<ContactLabel>, clients::Error> {
    let url = self.esi.url(&format!("corporations/{corporation_id}/contacts/labels/"));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn contacts(&self, corporation_id: i64) -> Result<Vec<Contact>, clients::Error> {
    let url = self.esi.url(&format!("corporations/{corporation_id}/contacts/"));
    self.esi.get_json_paginated(&url, Some(self.grant.access_token())).await
  }

  #[allow(dead_code)]
  pub async fn contract_bids(&self, corporation_id: i64, contract_id: i64) -> Result<Vec<ContractBid>, clients::Error> {
    let url = self
      .esi
      .url(&format!("corporations/{corporation_id}/contracts/{contract_id}/bids/"));
    self.esi.get_json_paginated(&url, Some(self.grant.access_token())).await
  }

  #[allow(dead_code)]
  pub async fn contract_items(
    &self,
    corporation_id: i64,
    contract_id: i64,
  ) -> Result<Vec<ContractItem>, clients::Error> {
    let url = self
      .esi
      .url(&format!("corporations/{corporation_id}/contracts/{contract_id}/items/"));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  #[allow(dead_code)]
  pub async fn contracts(&self, corporation_id: i64) -> Result<Vec<Contract>, clients::Error> {
    let url = self.esi.url(&format!("corporations/{corporation_id}/contracts/"));
    self.esi.get_json_paginated(&url, Some(self.grant.access_token())).await
  }

  pub async fn divisions(&self, corporation_id: i64) -> Result<CorporationDivisions, clients::Error> {
    let url = self.esi.url(&format!("corporations/{corporation_id}/divisions/"));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  #[allow(dead_code)]
  pub async fn industry_jobs(&self, corporation_id: i64) -> Result<Vec<IndustryJob>, clients::Error> {
    let url = self.esi.url(&format!("corporations/{corporation_id}/industry/jobs/"));
    self.esi.get_json_paginated(&url, Some(self.grant.access_token())).await
  }

  pub async fn member_roles(&self, corporation_id: i64) -> Result<Vec<MemberRole>, clients::Error> {
    let url = self.esi.url(&format!("corporations/{corporation_id}/roles/"));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn mining_extractions(&self, corporation_id: i64) -> Result<Vec<MiningExtraction>, clients::Error> {
    let url = self
      .esi
      .url(&format!("corporation/{corporation_id}/mining/extractions/"));
    self.esi.get_json_paginated(&url, Some(self.grant.access_token())).await
  }

  pub async fn recent_killmails(&self, corporation_id: i64) -> Result<Vec<RecentKillmail>, clients::Error> {
    let url = self
      .esi
      .url(&format!("corporations/{corporation_id}/killmails/recent/"));
    self.esi.get_json_paginated(&url, Some(self.grant.access_token())).await
  }

  pub async fn standings(&self, corporation_id: i64) -> Result<Vec<Standing>, clients::Error> {
    let url = self.esi.url(&format!("corporations/{corporation_id}/standings/"));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn structures(&self, corporation_id: i64) -> Result<Vec<CorporationStructure>, clients::Error> {
    let url = self.esi.url(&format!("corporations/{corporation_id}/structures/"));
    self.esi.get_json_paginated(&url, Some(self.grant.access_token())).await
  }

  pub async fn wallet_journal(
    &self,
    corporation_id: i64,
    division: i32,
  ) -> Result<Vec<CorporationWalletJournalEntry>, clients::Error> {
    let url = self
      .esi
      .url(&format!("corporations/{corporation_id}/wallets/{division}/journal/"));
    self.esi.get_json_paginated(&url, Some(self.grant.access_token())).await
  }

  pub async fn wallet_transactions(
    &self,
    corporation_id: i64,
    division: i32,
  ) -> Result<Vec<CorporationWalletTransaction>, clients::Error> {
    let url = self.esi.url(&format!(
      "corporations/{corporation_id}/wallets/{division}/transactions/"
    ));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn wallets(&self, corporation_id: i64) -> Result<Vec<CorporationWalletBalance>, clients::Error> {
    let url = self.esi.url(&format!("corporations/{corporation_id}/wallets/"));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
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
    let url = self.esi.url(&format!("corporations/{corporation_id}/"));
    self.esi.get_json(&url, None).await
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
          .and(path("/corporations/2000/assets/"))
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

    mod assets_names {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_batches_ids_into_chunks_of_at_most_a_thousand() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/corporations/2000/assets/names/"))
          .and(header("Authorization", "Bearer corp-token"))
          .respond_with(
            ResponseTemplate::new(200).set_body_raw(r#"[{"item_id":1,"name":"Named"}]"#, "application/json"),
          )
          .expect(2)
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);
        let ids: Vec<i64> = (1..=1500).collect();

        let names = esi
          .corporation_authenticated(&grant)
          .assets_names(2000, &ids)
          .await
          .unwrap();

        assert_eq!(names.len(), 2);
      }

      #[tokio::test]
      async fn it_posts_item_ids_with_the_bearer_token_and_parses_names() {
        let server = MockServer::start().await;
        let body = r#"[{"item_id":1000000016835,"name":"Corp Hauler"},{"item_id":1000000016836,"name":"Spare Parts"}]"#;
        Mock::given(method("POST"))
          .and(path("/corporations/2000/assets/names/"))
          .and(header("Authorization", "Bearer corp-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let names = esi
          .corporation_authenticated(&grant)
          .assets_names(2000, &[1000000016835, 1000000016836])
          .await
          .unwrap();

        assert_eq!(names.len(), 2);
        assert_eq!(names[0].item_id, 1000000016835);
        assert_eq!(names[0].name, "Corp Hauler");
        assert_eq!(names[1].name, "Spare Parts");
      }

      #[tokio::test]
      async fn it_skips_the_request_for_an_empty_id_list() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/corporations/2000/assets/names/"))
          .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
          .expect(0)
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let names = esi
          .corporation_authenticated(&grant)
          .assets_names(2000, &[])
          .await
          .unwrap();

        assert!(names.is_empty());
      }
    }

    mod blueprints {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_merges_all_pages() {
        let server = MockServer::start().await;
        let page_one = r#"[{"item_id":1000000000001,"type_id":962,"location_id":60003760,"location_flag":"CorpSAG1","quantity":-1,"material_efficiency":10,"time_efficiency":20,"runs":-1}]"#;
        let page_two = r#"[{"item_id":1000000000002,"type_id":963,"location_id":60003760,"location_flag":"CorpSAG1","quantity":1,"material_efficiency":2,"time_efficiency":4,"runs":150}]"#;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/blueprints/"))
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
          .and(path("/corporations/2000/blueprints/"))
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

        let blueprints = esi.corporation_authenticated(&grant).blueprints(2000).await.unwrap();

        assert_eq!(blueprints.len(), 2);
        assert_eq!(blueprints[0].item_id, 1000000000001);
        assert_eq!(blueprints[0].runs, -1);
        assert_eq!(blueprints[0].material_efficiency, 10);
        assert_eq!(blueprints[1].item_id, 1000000000002);
        assert_eq!(blueprints[1].runs, 150);
      }
    }

    mod contact_labels {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_labels() {
        let server = MockServer::start().await;
        let body = r#"[{"label_id":1,"label_name":"Friendlies"},{"label_id":2,"label_name":"Watchlist"}]"#;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/contacts/labels/"))
          .and(header("Authorization", "Bearer corp-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let labels = esi
          .corporation_authenticated(&grant)
          .contact_labels(2000)
          .await
          .unwrap();

        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].label_id, 1);
        assert_eq!(labels[0].label_name, "Friendlies");
        assert_eq!(labels[1].label_id, 2);
      }

      #[tokio::test]
      async fn it_surfaces_an_http_error_on_5xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/contacts/labels/"))
          .respond_with(ResponseTemplate::new(500))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let result = esi.corporation_authenticated(&grant).contact_labels(2000).await;

        assert!(matches!(result, Err(clients::Error::Http(_))));
      }
    }

    mod contacts {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_merges_all_pages() {
        let server = MockServer::start().await;
        let page_one =
          r#"[{"contact_id":95001,"contact_type":"character","is_watched":true,"label_ids":[1],"standing":7.5}]"#;
        let page_two = r#"[{"contact_id":98001,"contact_type":"corporation","standing":-10.0}]"#;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/contacts/"))
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
          .and(path("/corporations/2000/contacts/"))
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

        let contacts = esi.corporation_authenticated(&grant).contacts(2000).await.unwrap();

        assert_eq!(contacts.len(), 2);
        assert_eq!(contacts[0].contact_id, 95001);
        assert_eq!(contacts[0].contact_type, "character");
        assert_eq!(contacts[0].is_watched, Some(true));
        assert_eq!(contacts[0].label_ids, vec![1]);
        assert_eq!(contacts[1].contact_id, 98001);
        assert_eq!(contacts[1].standing, Some(-10.0));
      }

      #[tokio::test]
      async fn it_surfaces_an_http_error_on_5xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/contacts/"))
          .respond_with(ResponseTemplate::new(500))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let result = esi.corporation_authenticated(&grant).contacts(2000).await;

        assert!(matches!(result, Err(clients::Error::Http(_))));
      }
    }

    mod contract_bids {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_merges_all_pages() {
        let server = MockServer::start().await;
        let page_one = r#"[{"bid_id":1,"bidder_id":1001,"amount":1500000.0,"date_bid":"2024-01-01T00:00:00Z"}]"#;
        let page_two = r#"[{"bid_id":2,"bidder_id":1002,"amount":2000000.0,"date_bid":"2024-01-02T00:00:00Z"}]"#;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/contracts/9/bids/"))
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
          .and(path("/corporations/2000/contracts/9/bids/"))
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

        let bids = esi
          .corporation_authenticated(&grant)
          .contract_bids(2000, 9)
          .await
          .unwrap();

        assert_eq!(bids.len(), 2);
        assert_eq!(bids[0].bid_id, 1);
        assert_eq!(bids[0].amount, 1500000.0);
        assert_eq!(bids[1].bid_id, 2);
      }
    }

    mod contract_items {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_items() {
        let server = MockServer::start().await;
        let body = r#"[{"record_id":100,"type_id":587,"quantity":1,"raw_quantity":-1,"is_singleton":true,"is_included":true},{"record_id":101,"type_id":34,"quantity":500,"is_singleton":false,"is_included":false}]"#;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/contracts/9/items/"))
          .and(header("Authorization", "Bearer corp-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let items = esi
          .corporation_authenticated(&grant)
          .contract_items(2000, 9)
          .await
          .unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].record_id, 100);
        assert_eq!(items[0].type_id, 587);
        assert_eq!(items[0].raw_quantity, Some(-1));
        assert!(items[0].is_singleton);
        assert!(items[0].is_included);
        assert_eq!(items[1].record_id, 101);
        assert_eq!(items[1].raw_quantity, None);
        assert!(!items[1].is_included);
      }
    }

    mod contracts {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_merges_all_pages() {
        let server = MockServer::start().await;
        let page_one = r#"[{"contract_id":1,"issuer_id":1001,"issuer_corporation_id":2000,"assignee_id":3003,"acceptor_id":0,"type":"item_exchange","status":"outstanding","for_corporation":true,"availability":"corporation","date_issued":"2024-01-01T00:00:00Z","date_expired":"2024-02-01T00:00:00Z","price":1000.0}]"#;
        let page_two = r#"[{"contract_id":2,"issuer_id":1002,"issuer_corporation_id":2000,"assignee_id":0,"acceptor_id":0,"type":"courier","status":"in_progress","for_corporation":true,"availability":"corporation","date_issued":"2024-01-03T00:00:00Z","date_expired":"2024-02-03T00:00:00Z","reward":500.0,"collateral":100.0,"volume":1500.0}]"#;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/contracts/"))
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
          .and(path("/corporations/2000/contracts/"))
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

        let contracts = esi.corporation_authenticated(&grant).contracts(2000).await.unwrap();

        assert_eq!(contracts.len(), 2);
        assert_eq!(contracts[0].contract_id, 1);
        assert_eq!(contracts[0].contract_type.as_deref(), Some("item_exchange"));
        assert_eq!(contracts[0].for_corporation, Some(true));
        assert_eq!(contracts[1].contract_id, 2);
        assert_eq!(contracts[1].reward, Some(500.0));
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
          .and(path("/corporations/2000/divisions/"))
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

    mod industry_jobs {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_merges_all_pages() {
        let server = MockServer::start().await;
        let page_one = r#"[{"activity_id":1,"blueprint_id":1000000000001,"blueprint_location_id":60003760,"blueprint_type_id":962,"duration":3600,"end_date":"2026-01-01T01:00:00Z","facility_id":60003760,"installer_id":42,"job_id":10,"output_location_id":60003760,"runs":5,"start_date":"2026-01-01T00:00:00Z","status":"active"}]"#;
        let page_two = r#"[{"activity_id":9,"blueprint_id":1000000000002,"blueprint_location_id":60003760,"blueprint_type_id":963,"cost":250.0,"duration":7200,"end_date":"2026-01-02T02:00:00Z","facility_id":60003760,"installer_id":43,"job_id":11,"output_location_id":60003760,"runs":1,"start_date":"2026-01-02T00:00:00Z","status":"active"}]"#;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/industry/jobs/"))
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
          .and(path("/corporations/2000/industry/jobs/"))
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

        let jobs = esi.corporation_authenticated(&grant).industry_jobs(2000).await.unwrap();

        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].job_id, 10);
        assert_eq!(jobs[0].activity_id, 1);
        assert_eq!(jobs[0].cost, None);
        assert_eq!(jobs[1].job_id, 11);
        assert_eq!(jobs[1].activity_id, 9);
        assert_eq!(jobs[1].cost, Some(250.0));
      }

      #[tokio::test]
      async fn it_surfaces_an_http_error_when_the_token_lacks_factory_manager_role() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/industry/jobs/"))
          .respond_with(ResponseTemplate::new(403))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let result = esi.corporation_authenticated(&grant).industry_jobs(2000).await;

        assert!(matches!(result, Err(clients::Error::Http(_))));
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
          .and(path("/corporations/2000/roles/"))
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

    mod mining_extractions {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_extractions() {
        let server = MockServer::start().await;
        let body = r#"[{"chunk_arrival_time":"2026-06-20T00:00:00Z","extraction_start_time":"2026-06-13T00:00:00Z","moon_id":40000001,"natural_decay_time":"2026-06-21T00:00:00Z","structure_id":1021000000001}]"#;
        Mock::given(method("GET"))
          .and(path("/corporation/2000/mining/extractions/"))
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

        let extractions = esi
          .corporation_authenticated(&grant)
          .mining_extractions(2000)
          .await
          .unwrap();

        assert_eq!(extractions.len(), 1);
        assert_eq!(extractions[0].moon_id, 40000001);
        assert_eq!(extractions[0].structure_id, 1021000000001);
        assert_eq!(extractions[0].chunk_arrival_time, "2026-06-20T00:00:00Z");
      }

      #[tokio::test]
      async fn it_surfaces_an_http_error_when_the_token_lacks_station_manager_role() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/corporation/2000/mining/extractions/"))
          .respond_with(ResponseTemplate::new(403))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let result = esi.corporation_authenticated(&grant).mining_extractions(2000).await;

        assert!(matches!(result, Err(clients::Error::Http(_))));
      }
    }

    mod recent_killmails {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_merges_all_pages() {
        let server = MockServer::start().await;
        let page_one = r#"[{"killmail_id":100,"killmail_hash":"abc123"}]"#;
        let page_two = r#"[{"killmail_id":200,"killmail_hash":"def456"}]"#;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/killmails/recent/"))
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
          .and(path("/corporations/2000/killmails/recent/"))
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

        let killmails = esi
          .corporation_authenticated(&grant)
          .recent_killmails(2000)
          .await
          .unwrap();

        assert_eq!(killmails.len(), 2);
        assert_eq!(killmails[0].killmail_id, 100);
        assert_eq!(killmails[0].killmail_hash, "abc123");
        assert_eq!(killmails[1].killmail_id, 200);
      }

      #[tokio::test]
      async fn it_surfaces_an_http_error_when_the_token_lacks_director_role() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/killmails/recent/"))
          .respond_with(ResponseTemplate::new(403))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let result = esi.corporation_authenticated(&grant).recent_killmails(2000).await;

        assert!(matches!(result, Err(clients::Error::Http(_))));
      }
    }

    mod standings {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_standings() {
        let server = MockServer::start().await;
        let body = r#"[{"from_id":500003,"from_type":"faction","standing":7.5},{"from_id":1000125,"from_type":"npc_corp","standing":-2.5}]"#;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/standings/"))
          .and(header("Authorization", "Bearer corp-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let standings = esi.corporation_authenticated(&grant).standings(2000).await.unwrap();

        assert_eq!(standings.len(), 2);
        assert_eq!(standings[0].from_id, 500003);
        assert_eq!(standings[0].from_type, "faction");
        assert_eq!(standings[0].standing, 7.5);
        assert_eq!(standings[1].from_id, 1000125);
        assert_eq!(standings[1].standing, -2.5);
      }

      #[tokio::test]
      async fn it_surfaces_an_http_error_on_5xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/standings/"))
          .respond_with(ResponseTemplate::new(500))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let result = esi.corporation_authenticated(&grant).standings(2000).await;

        assert!(matches!(result, Err(clients::Error::Http(_))));
      }
    }

    mod structures {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_merges_all_pages() {
        let server = MockServer::start().await;
        let page_one = r#"[{"corporation_id":2000,"structure_id":1021000000001,"system_id":30000142,"type_id":35833,"name":"Jita Keepstar","services":[{"name":"Manufacturing","state":"online"}],"state":"shield_vulnerable"}]"#;
        let page_two = r#"[{"corporation_id":2000,"structure_id":1021000000002,"system_id":30002187,"type_id":35825}]"#;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/structures/"))
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
          .and(path("/corporations/2000/structures/"))
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

        let structures = esi.corporation_authenticated(&grant).structures(2000).await.unwrap();

        assert_eq!(structures.len(), 2);
        assert_eq!(structures[0].structure_id, 1021000000001);
        assert_eq!(structures[0].name.as_deref(), Some("Jita Keepstar"));
        assert_eq!(structures[0].system_id, 30000142);
        assert_eq!(structures[1].structure_id, 1021000000002);
        assert_eq!(structures[1].name, None);
      }

      #[tokio::test]
      async fn it_surfaces_an_http_error_when_the_token_lacks_station_manager_role() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/corporations/2000/structures/"))
          .respond_with(ResponseTemplate::new(403))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("corp-token", 42);

        let result = esi.corporation_authenticated(&grant).structures(2000).await;

        assert!(matches!(result, Err(clients::Error::Http(_))));
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
          .and(path("/corporations/2000/wallets/1/journal/"))
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
          .and(path("/corporations/2000/wallets/1/journal/"))
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
          .and(path("/corporations/2000/wallets/1/transactions/"))
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
          .and(path("/corporations/2000/wallets/"))
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
          .and(path("/corporations/2000/"))
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
          .and(path("/corporations/2000/"))
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
