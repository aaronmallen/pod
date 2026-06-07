use crate::clients::{
  self,
  esi::{Client as EsiClient, models::alliance::AllianceInfo},
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

  pub async fn info(&self, alliance_id: i64) -> Result<AllianceInfo, clients::Error> {
    let url = self.esi.url(&format!("v4/alliances/{alliance_id}/"));
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

  mod info {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_alliance_info() {
      let server = MockServer::start().await;
      let body = r#"{
        "creator_corporation_id": 45678,
        "creator_id": 12345,
        "date_founded": "2010-11-01T12:34:56Z",
        "executor_corporation_id": 98765,
        "name": "Test Alliance",
        "ticker": "TEST"
      }"#;
      Mock::given(method("GET"))
        .and(path("/v4/alliances/99/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let info = esi.alliance().info(99).await.unwrap();

      assert_eq!(info.creator_corporation_id, 45678);
      assert_eq!(info.executor_corporation_id, Some(98765));
      assert_eq!(info.name, "Test Alliance");
      assert_eq!(info.ticker, "TEST");
    }

    #[tokio::test]
    async fn it_returns_http_error_on_4xx() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v4/alliances/99/"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let result = esi.alliance().info(99).await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }
  }
}
