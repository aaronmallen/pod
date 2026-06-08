use crate::clients::{
  self,
  esi::{
    Client as EsiClient,
    models::character::{
      Asset, Attributes, CharacterInfo, CharacterSkills, Clones, Contact, ContactLabel, Contract, Location, MailBody,
      MailHeader, MarkReadRequest, MarketOrder, Notification, Online, RecentKillmail, SendMailRequest, Ship,
      SkillQueueEntry, Standing, WalletJournalEntry, WalletTransaction,
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

  pub async fn assets(&self) -> Result<Vec<Asset>, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/assets/", self.grant.character_id()));
    self.esi.get_json_paginated(&url, Some(self.grant.access_token())).await
  }

  pub async fn attributes(&self) -> Result<Attributes, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/attributes/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn clones(&self) -> Result<Clones, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/clones/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn contact_labels(&self) -> Result<Vec<ContactLabel>, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/contacts/labels/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn contacts(&self) -> Result<Vec<Contact>, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/contacts/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn contracts(&self) -> Result<Vec<Contract>, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/contracts/", self.grant.character_id()));
    self.esi.get_json_paginated(&url, Some(self.grant.access_token())).await
  }

  pub async fn implants(&self) -> Result<Vec<i64>, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/implants/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn location(&self) -> Result<Location, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/location/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn mail(&self) -> Result<Vec<MailHeader>, clients::Error> {
    let url = self.esi.url(&format!("characters/{}/mail/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn mail_body(&self, mail_id: i64) -> Result<MailBody, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/mail/{mail_id}/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn mark_read(&self, mail_id: i64, request: &MarkReadRequest) -> Result<(), clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/mail/{mail_id}/", self.grant.character_id()));
    self.esi.put_empty(&url, request, self.grant.access_token()).await
  }

  pub async fn notifications(&self) -> Result<Vec<Notification>, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/notifications/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn online(&self) -> Result<Online, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/online/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn orders(&self) -> Result<Vec<MarketOrder>, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/orders/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  #[allow(dead_code)]
  pub async fn recent_killmails(&self) -> Result<Vec<RecentKillmail>, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/killmails/recent/", self.grant.character_id()));
    self.esi.get_json_paginated(&url, Some(self.grant.access_token())).await
  }

  pub async fn send_mail(&self, request: &SendMailRequest) -> Result<i64, clients::Error> {
    let url = self.esi.url(&format!("characters/{}/mail/", self.grant.character_id()));
    self.esi.post_json(&url, request, self.grant.access_token()).await
  }

  pub async fn ship(&self) -> Result<Ship, clients::Error> {
    let url = self.esi.url(&format!("characters/{}/ship/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn skill_queue(&self) -> Result<Vec<SkillQueueEntry>, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/skillqueue/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn skills(&self) -> Result<CharacterSkills, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/skills/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn standings(&self) -> Result<Vec<Standing>, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/standings/", self.grant.character_id()));
    self.esi.get_json(&url, Some(self.grant.access_token())).await
  }

  pub async fn wallet_journal(&self) -> Result<Vec<WalletJournalEntry>, clients::Error> {
    let url = self
      .esi
      .url(&format!("characters/{}/wallet/journal/", self.grant.character_id()));
    self.esi.get_json_paginated(&url, Some(self.grant.access_token())).await
  }

  pub async fn wallet_transactions(&self) -> Result<Vec<WalletTransaction>, clients::Error> {
    let url = self.esi.url(&format!(
      "characters/{}/wallet/transactions/",
      self.grant.character_id()
    ));
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

  pub async fn public_info(&self, character_id: i64) -> Result<CharacterInfo, clients::Error> {
    let url = self.esi.url(&format!("characters/{character_id}/"));
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
      async fn it_merges_all_pages() {
        let server = MockServer::start().await;
        let page1 = r#"[{"is_singleton":true,"item_id":1,"location_flag":"Hangar","location_id":60,"location_type":"station","quantity":1,"type_id":587}]"#;
        let page2 = r#"[{"is_singleton":false,"item_id":2,"location_flag":"Hangar","location_id":60,"location_type":"station","quantity":5,"type_id":34}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/assets/"))
          .and(wiremock::matchers::query_param("page", "1"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "2")
              .set_body_raw(page1, "application/json"),
          )
          .mount(&server)
          .await;
        Mock::given(method("GET"))
          .and(path("/characters/42/assets/"))
          .and(wiremock::matchers::query_param("page", "2"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "2")
              .set_body_raw(page2, "application/json"),
          )
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("token", 42);

        let assets = esi.character_authenticated(&grant).assets().await.unwrap();

        assert_eq!(assets.len(), 2);
      }

      #[tokio::test]
      async fn it_sends_the_bearer_token() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/characters/42/assets/"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "1")
              .set_body_raw("[]", "application/json"),
          )
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);

        let assets = esi.character_authenticated(&grant).assets().await.unwrap();

        assert_eq!(assets.len(), 0);
      }
    }

    mod attributes {
      use pretty_assertions::assert_eq;

      use super::*;

      const ATTRIBUTES_FIXTURE: &str = include_str!("../../../test/fixtures/esi/character_attributes.json");

      #[tokio::test]
      async fn it_sends_the_bearer_token_to_the_v1_attributes_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/characters/42/attributes/"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(ATTRIBUTES_FIXTURE, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);

        let attributes = esi.character_authenticated(&grant).attributes().await.unwrap();

        assert_eq!(attributes.intelligence, 22);
        assert_eq!(attributes.memory, 21);
        assert_eq!(attributes.bonus_remaps, 2);
        assert_eq!(attributes.last_remap_date.as_deref(), Some("2023-04-01T12:00:00Z"));
      }

      #[tokio::test]
      async fn it_defaults_remap_fields_for_a_never_remapped_pilot() {
        let server = MockServer::start().await;
        let body = r#"{"charisma":19,"intelligence":20,"memory":20,"perception":20,"willpower":20}"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/attributes/"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("token", 42);

        let attributes = esi.character_authenticated(&grant).attributes().await.unwrap();

        assert_eq!(attributes.bonus_remaps, 0);
        assert!(attributes.last_remap_date.is_none());
        assert!(attributes.accrued_remap_cooldown_date.is_none());
      }
    }

    mod clones {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_home_and_jump_clones() {
        let server = MockServer::start().await;
        let body = r#"{
          "home_location":{"location_id":60003760,"location_type":"station"},
          "jump_clones":[{"implants":[9899,9941],"jump_clone_id":1,"location_id":60003760,"location_type":"station","name":"Backup"}],
          "last_clone_jump_date":"2024-01-01T00:00:00Z",
          "last_station_change_date":"2024-02-01T00:00:00Z"
        }"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/clones/"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);

        let clones = esi.character_authenticated(&grant).clones().await.unwrap();

        assert_eq!(clones.home_location.location_id, Some(60003760));
        assert_eq!(clones.home_location.location_type.as_deref(), Some("station"));
        assert_eq!(clones.jump_clones.len(), 1);
        assert_eq!(clones.jump_clones[0].jump_clone_id, 1);
        assert_eq!(clones.jump_clones[0].implants, vec![9899, 9941]);
        assert_eq!(clones.jump_clones[0].name.as_deref(), Some("Backup"));
      }
    }

    mod contact_labels {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_labels() {
        let server = MockServer::start().await;
        let body = r#"[{"label_id":1,"label_name":"Friends"},{"label_id":2,"label_name":"Enemies"}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/contacts/labels/"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);

        let labels = esi.character_authenticated(&grant).contact_labels().await.unwrap();

        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].label_id, 1);
        assert_eq!(labels[0].label_name, "Friends");
      }
    }

    mod contacts {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_contacts() {
        let server = MockServer::start().await;
        let body =
          r#"[{"contact_id":1001,"contact_type":"character","is_watched":true,"label_ids":[1],"standing":7.5}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/contacts/"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);

        let contacts = esi.character_authenticated(&grant).contacts().await.unwrap();

        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].contact_id, 1001);
        assert_eq!(contacts[0].contact_type, "character");
        assert_eq!(contacts[0].is_watched, Some(true));
        assert_eq!(contacts[0].label_ids, vec![1]);
        assert_eq!(contacts[0].standing, Some(7.5));
      }

      #[tokio::test]
      async fn it_defaults_optional_fields_for_an_unrated_contact() {
        let server = MockServer::start().await;
        let body = r#"[{"contact_id":2002,"contact_type":"corporation"}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/contacts/"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("token", 42);

        let contacts = esi.character_authenticated(&grant).contacts().await.unwrap();

        assert_eq!(contacts.len(), 1);
        assert!(contacts[0].standing.is_none());
        assert!(contacts[0].is_watched.is_none());
        assert!(contacts[0].label_ids.is_empty());
      }
    }

    mod contracts {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_merges_all_pages() {
        let server = MockServer::start().await;
        let page1 = r#"[{"contract_id":1,"type":"item_exchange","status":"outstanding","price":1000.0}]"#;
        let page2 = r#"[{"contract_id":2,"type":"courier","status":"in_progress","reward":500.0,"collateral":100.0,"volume":1500.0}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/contracts/"))
          .and(wiremock::matchers::query_param("page", "1"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "2")
              .set_body_raw(page1, "application/json"),
          )
          .mount(&server)
          .await;
        Mock::given(method("GET"))
          .and(path("/characters/42/contracts/"))
          .and(wiremock::matchers::query_param("page", "2"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "2")
              .set_body_raw(page2, "application/json"),
          )
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("token", 42);

        let contracts = esi.character_authenticated(&grant).contracts().await.unwrap();

        assert_eq!(contracts.len(), 2);
      }

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_deserializes_a_contract() {
        let server = MockServer::start().await;
        let body = r#"[{"contract_id":9,"type":"courier","status":"outstanding","issuer_id":1001,"assignee_id":2002,"reward":1234.5,"collateral":6789.0,"volume":250.0,"date_issued":"2024-01-01T00:00:00Z","for_corporation":false}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/contracts/"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "1")
              .set_body_raw(body, "application/json"),
          )
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);

        let contracts = esi.character_authenticated(&grant).contracts().await.unwrap();

        assert_eq!(contracts.len(), 1);
        assert_eq!(contracts[0].contract_id, 9);
        assert_eq!(contracts[0].contract_type.as_deref(), Some("courier"));
        assert_eq!(contracts[0].status.as_deref(), Some("outstanding"));
        assert_eq!(contracts[0].issuer_id, Some(1001));
        assert_eq!(contracts[0].reward, Some(1234.5));
        assert_eq!(contracts[0].for_corporation, Some(false));
      }
    }

    mod implants {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_a_bare_array_of_type_ids() {
        let server = MockServer::start().await;
        let body = r#"[9899,9941,9942]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/implants/"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);

        let implants = esi.character_authenticated(&grant).implants().await.unwrap();

        assert_eq!(implants, vec![9899, 9941, 9942]);
      }
    }

    mod location {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_the_location() {
        let server = MockServer::start().await;
        let body = r#"{"solar_system_id":30000142,"station_id":60003760}"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/location/"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);

        let location = esi.character_authenticated(&grant).location().await.unwrap();

        assert_eq!(location.solar_system_id, 30000142);
        assert_eq!(location.station_id, Some(60003760));
        assert_eq!(location.structure_id, None);
      }
    }

    mod mail {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_mail_headers() {
        let server = MockServer::start().await;
        let body = r#"[{"mail_id":7,"from":1001,"recipients":[{"recipient_id":2002,"recipient_type":"character"},{"recipient_id":3003,"recipient_type":"mailing_list"}],"subject":"Hello","timestamp":"2024-01-01T00:00:00Z","is_read":true,"labels":[1,4]}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/mail/"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);

        let headers = esi.character_authenticated(&grant).mail().await.unwrap();

        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].mail_id, 7);
        assert_eq!(headers[0].from, Some(1001));
        assert_eq!(headers[0].subject.as_deref(), Some("Hello"));
        assert_eq!(headers[0].timestamp.as_deref(), Some("2024-01-01T00:00:00Z"));
        assert_eq!(headers[0].is_read, Some(true));
        assert_eq!(headers[0].labels, vec![1, 4]);
        assert_eq!(headers[0].recipients.len(), 2);
        assert_eq!(headers[0].recipients[0].recipient_id, 2002);
        assert_eq!(headers[0].recipients[0].recipient_type, "character");
        assert_eq!(headers[0].recipients[1].recipient_type, "mailing_list");
      }

      #[tokio::test]
      async fn it_defaults_optional_fields_for_a_system_mail() {
        let server = MockServer::start().await;
        let body = r#"[{"mail_id":8}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/mail/"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("token", 42);

        let headers = esi.character_authenticated(&grant).mail().await.unwrap();

        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].mail_id, 8);
        assert!(headers[0].from.is_none());
        assert!(headers[0].is_read.is_none());
        assert!(headers[0].subject.is_none());
        assert!(headers[0].labels.is_empty());
        assert!(headers[0].recipients.is_empty());
      }
    }

    mod mail_body {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_the_html_body() {
        let server = MockServer::start().await;
        let body = r#"{"body":"<p>Greetings, capsuleer.</p>","from":1001,"labels":[1],"read":true,"subject":"Hello","timestamp":"2024-01-01T00:00:00Z","recipients":[{"recipient_id":2002,"recipient_type":"character"}]}"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/mail/7/"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);

        let mail = esi.character_authenticated(&grant).mail_body(7).await.unwrap();

        assert_eq!(mail.body, "<p>Greetings, capsuleer.</p>");
      }
    }

    mod mark_read {
      use serde_json::{Value, json};
      use wiremock::matchers::body_json;

      use super::*;

      #[tokio::test]
      async fn it_puts_the_read_flag_with_the_bearer_token() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
          .and(path("/characters/42/mail/7/"))
          .and(header("Authorization", "Bearer secret-token"))
          .and(body_json(json!({"read": true})))
          .respond_with(ResponseTemplate::new(204))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);
        let request = MarkReadRequest {
          labels: None,
          read: Some(true),
        };

        let result = esi.character_authenticated(&grant).mark_read(7, &request).await;

        assert!(result.is_ok());
      }

      #[tokio::test]
      async fn it_omits_unset_fields_from_the_body() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
          .and(path("/characters/42/mail/7/"))
          .respond_with(ResponseTemplate::new(204))
          .mount(&server)
          .await;
        let request = MarkReadRequest {
          labels: None,
          read: Some(true),
        };

        let serialized: Value = serde_json::to_value(&request).unwrap();

        assert_eq!(serialized, json!({"read": true}));
      }
    }

    mod orders {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_maps_a_buy_order() {
        let server = MockServer::start().await;
        let body = r#"[{"order_id":1001,"type_id":34,"region_id":10000002,"location_id":60003760,"range":"region","is_buy_order":true,"price":5.5,"volume_remain":100,"volume_total":200,"min_volume":1,"escrow":550.0,"duration":90,"issued":"2026-06-01T12:00:00Z"}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/orders/"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);

        let orders = esi.character_authenticated(&grant).orders().await.unwrap();

        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].order_id, 1001);
        assert_eq!(orders[0].type_id, 34);
        assert_eq!(orders[0].region_id, 10000002);
        assert_eq!(orders[0].location_id, 60003760);
        assert_eq!(orders[0].range, "region");
        assert!(orders[0].is_buy_order);
        assert_eq!(orders[0].price, 5.5);
        assert_eq!(orders[0].volume_remain, 100);
        assert_eq!(orders[0].volume_total, 200);
        assert_eq!(orders[0].escrow, 550.0);
        assert_eq!(orders[0].duration, 90);
        assert_eq!(orders[0].issued, "2026-06-01T12:00:00Z");
      }

      #[tokio::test]
      async fn it_defaults_is_buy_order_and_escrow_for_a_sell_order() {
        let server = MockServer::start().await;
        let body = r#"[{"order_id":2002,"type_id":35,"region_id":10000002,"location_id":60003760,"range":"station","price":12.3,"volume_remain":5,"volume_total":5,"duration":30,"issued":"2026-06-02T00:00:00Z"}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/orders/"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("token", 42);

        let orders = esi.character_authenticated(&grant).orders().await.unwrap();

        assert_eq!(orders.len(), 1);
        assert!(!orders[0].is_buy_order);
        assert_eq!(orders[0].escrow, 0.0);
        assert!(orders[0].min_volume.is_none());
      }
    }

    mod send_mail {
      use pretty_assertions::assert_eq;
      use serde_json::{Value, json};
      use wiremock::matchers::body_json;

      use super::*;
      use crate::clients::esi::models::character::SendMailRecipient;

      #[tokio::test]
      async fn it_posts_the_send_body_and_returns_the_new_mail_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/characters/42/mail/"))
          .and(header("Authorization", "Bearer secret-token"))
          .and(body_json(json!({
            "body": "Form up at 19:00.",
            "recipients": [{"recipient_id": 2002, "recipient_type": "character"}],
            "subject": "CTA"
          })))
          .respond_with(ResponseTemplate::new(201).set_body_raw("123456789", "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);
        let request = SendMailRequest {
          approved_cost: None,
          body: "Form up at 19:00.".to_string(),
          recipients: vec![SendMailRecipient {
            recipient_id: 2002,
            recipient_type: "character".to_string(),
          }],
          subject: "CTA".to_string(),
        };

        let mail_id = esi.character_authenticated(&grant).send_mail(&request).await.unwrap();

        assert_eq!(mail_id, 123456789);
      }

      #[tokio::test]
      async fn it_serializes_recipient_types_for_every_target_kind() {
        let request = SendMailRequest {
          approved_cost: Some(50),
          body: "hi".to_string(),
          recipients: vec![
            SendMailRecipient {
              recipient_id: 1,
              recipient_type: "character".to_string(),
            },
            SendMailRecipient {
              recipient_id: 2,
              recipient_type: "corporation".to_string(),
            },
            SendMailRecipient {
              recipient_id: 3,
              recipient_type: "alliance".to_string(),
            },
            SendMailRecipient {
              recipient_id: 4,
              recipient_type: "mailing_list".to_string(),
            },
          ],
          subject: "s".to_string(),
        };

        let serialized: Value = serde_json::to_value(&request).unwrap();

        assert_eq!(serialized["approved_cost"], 50);
        assert_eq!(serialized["recipients"][0]["recipient_type"], "character");
        assert_eq!(serialized["recipients"][1]["recipient_type"], "corporation");
        assert_eq!(serialized["recipients"][2]["recipient_type"], "alliance");
        assert_eq!(serialized["recipients"][3]["recipient_type"], "mailing_list");
      }
    }

    mod notifications {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_notifications() {
        let server = MockServer::start().await;
        let body = r#"[{"notification_id":7,"type":"KillReportFinalBlow","sender_id":1001,"sender_type":"character","timestamp":"2024-01-01T00:00:00Z","is_read":true,"text":"body"}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/notifications/"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);

        let notifications = esi.character_authenticated(&grant).notifications().await.unwrap();

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].notification_id, 7);
        assert_eq!(notifications[0].notif_type, "KillReportFinalBlow");
        assert_eq!(notifications[0].sender_id, Some(1001));
        assert_eq!(notifications[0].is_read, Some(true));
        assert_eq!(notifications[0].text.as_deref(), Some("body"));
      }

      #[tokio::test]
      async fn it_defaults_optional_fields_for_a_system_notification() {
        let server = MockServer::start().await;
        let body = r#"[{"notification_id":8,"type":"TutorialMsg","timestamp":"2024-02-01T00:00:00Z"}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/notifications/"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("token", 42);

        let notifications = esi.character_authenticated(&grant).notifications().await.unwrap();

        assert!(notifications[0].is_read.is_none());
        assert!(notifications[0].sender_id.is_none());
        assert!(notifications[0].text.is_none());
      }
    }

    mod recent_killmails {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_killmail_references() {
        let server = MockServer::start().await;
        let body = r#"[{"killmail_id":100,"killmail_hash":"abc123"}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/killmails/recent/"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "1")
              .set_body_raw(body, "application/json"),
          )
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);

        let killmails = esi.character_authenticated(&grant).recent_killmails().await.unwrap();

        assert_eq!(killmails.len(), 1);
        assert_eq!(killmails[0].killmail_id, 100);
        assert_eq!(killmails[0].killmail_hash, "abc123");
      }
    }

    mod skills {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_returns_the_skill_sheet() {
        let server = MockServer::start().await;
        let body = r#"{"skills":[{"active_skill_level":5,"skill_id":3300,"skillpoints_in_skill":256000,"trained_skill_level":5}],"total_sp":256000}"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/skills/"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("token", 42);

        let skills = esi.character_authenticated(&grant).skills().await.unwrap();

        assert_eq!(skills.total_sp, 256000);
        assert_eq!(skills.skills.len(), 1);
      }
    }

    mod standings {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_sends_the_bearer_token_and_returns_standings() {
        let server = MockServer::start().await;
        let body = r#"[{"from_id":500001,"from_type":"faction","standing":5.0},{"from_id":1000035,"from_type":"npc_corp","standing":-2.5}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/standings/"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("secret-token", 42);

        let standings = esi.character_authenticated(&grant).standings().await.unwrap();

        assert_eq!(standings.len(), 2);
        assert_eq!(standings[0].from_id, 500001);
        assert_eq!(standings[0].from_type, "faction");
        assert_eq!(standings[0].standing, 5.0);
        assert_eq!(standings[1].standing, -2.5);
      }
    }

    mod wallet_journal {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_deserializes_entries_that_omit_amount_and_balance() {
        let server = MockServer::start().await;
        let body = r#"[{"date":"2024-01-01T00:00:00Z","description":"corp tax","id":1,"ref_type":"corporation_account_withdrawal"}]"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/wallet/journal/"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "1")
              .set_body_raw(body, "application/json"),
          )
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;
        let grant = Grant::new_test("token", 42);

        let entries = esi.character_authenticated(&grant).wallet_journal().await.unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].amount, None);
        assert_eq!(entries[0].balance, None);
      }
    }
  }

  mod public_client {
    use super::*;

    mod public_info {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_returns_character_info() {
        let server = MockServer::start().await;
        let body = r#"{
          "birthday": "2015-03-24T11:37:00Z",
          "bloodline_id": 3,
          "corporation_id": 109299958,
          "gender": "male",
          "name": "Test Pilot",
          "race_id": 2
        }"#;
        Mock::given(method("GET"))
          .and(path("/characters/42/"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;

        let info = esi.character().public_info(42).await.unwrap();

        assert_eq!(info.corporation_id, 109299958);
        assert_eq!(info.name, "Test Pilot");
      }

      #[tokio::test]
      async fn it_returns_http_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/characters/42/"))
          .respond_with(ResponseTemplate::new(404))
          .mount(&server)
          .await;
        let esi = make_esi(&server.uri()).await;

        let result = esi.character().public_info(42).await;

        assert!(matches!(result, Err(clients::Error::Http(_))));
      }
    }
  }
}
