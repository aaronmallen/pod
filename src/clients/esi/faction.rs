use crate::clients::{
  self,
  esi::{Client as EsiClient, models::faction::Faction},
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

  pub async fn list(&self) -> Result<Vec<Faction>, clients::Error> {
    let url = self.esi.url("v2/universe/factions/");
    self.esi.http().get_json(&url, None).await
  }
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::{clients::http, store};

  async fn make_esi(base_url: &str) -> EsiClient {
    let db = store::open_test().await.unwrap();
    let cache = http::Cache::new(db);
    let http = http::Client::builder(cache).build();
    EsiClient::with_base_url(http, base_url)
  }

  mod list {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_factions() {
      let server = MockServer::start().await;
      let body = r#"[{"corporation_id":1000084,"description":"The Amarr Empire.","faction_id":500003,"is_unique":true,"militia_corporation_id":500003,"name":"Amarr Empire","size_factor":5.0,"solar_system_id":30002187,"station_count":1031,"station_system_count":508}]"#;
      Mock::given(method("GET"))
        .and(path("/v2/universe/factions/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let factions = esi.faction().list().await.unwrap();

      assert_eq!(factions.len(), 1);
      assert_eq!(factions[0].name, "Amarr Empire");
      assert_eq!(factions[0].faction_id, 500003);
    }

    #[tokio::test]
    async fn it_returns_http_error_on_5xx() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v2/universe/factions/"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let result = esi.faction().list().await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }
  }
}
