use crate::clients::{
  self,
  esi::{Client as EsiClient, models::killmail::Killmail},
};

pub struct Client<'a> {
  esi: &'a EsiClient,
}

impl<'a> Client<'a> {
  pub fn new(esi: &'a EsiClient) -> Self {
    Self {
      esi,
    }
  }

  pub async fn detail(&self, killmail_id: i64, hash: &str) -> Result<Killmail, clients::Error> {
    let url = self.esi.url(&format!("killmails/{killmail_id}/{hash}/"));
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

  mod detail {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_the_killmail_detail() {
      let server = MockServer::start().await;
      let body = r#"{
        "killmail_id": 100,
        "killmail_time": "2024-01-01T00:00:00Z",
        "solar_system_id": 30000142,
        "victim": {"character_id": 2002, "corporation_id": 3003, "ship_type_id": 587},
        "attackers": [
          {"character_id": 42, "final_blow": true},
          {"final_blow": false}
        ]
      }"#;
      Mock::given(method("GET"))
        .and(path("/killmails/100/abc123/"))
        .and(header("X-Compatibility-Date", "2026-06-08"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let killmail = esi.killmail().detail(100, "abc123").await.unwrap();

      assert_eq!(killmail.killmail_id, 100);
      assert_eq!(killmail.solar_system_id, 30000142);
      assert_eq!(killmail.victim.ship_type_id, 587);
      assert_eq!(killmail.victim.character_id, Some(2002));
      assert_eq!(killmail.attackers.len(), 2);
      assert!(killmail.attackers[0].final_blow);
      assert_eq!(killmail.attackers[0].character_id, Some(42));
    }

    #[tokio::test]
    async fn it_defaults_optional_victim_fields_for_a_structure_kill() {
      let server = MockServer::start().await;
      let body = r#"{
        "killmail_id": 200,
        "killmail_time": "2024-02-01T00:00:00Z",
        "solar_system_id": 30000142,
        "victim": {"ship_type_id": 35832}
      }"#;
      Mock::given(method("GET"))
        .and(path("/killmails/200/def456/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let killmail = esi.killmail().detail(200, "def456").await.unwrap();

      assert!(killmail.victim.character_id.is_none());
      assert!(killmail.victim.corporation_id.is_none());
      assert!(killmail.attackers.is_empty());
    }
  }
}
