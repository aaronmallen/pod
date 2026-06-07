use crate::clients::{
  self,
  esi::{Client as EsiClient, models::races::Race},
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

  pub async fn list(&self) -> Result<Vec<Race>, clients::Error> {
    let url = self.esi.url("v1/universe/races/");
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
    async fn it_returns_races() {
      let server = MockServer::start().await;
      let body = r#"[{"alliance_id":500001,"description":"Founded on the tenets of patriotism and hard work.","name":"Caldari","race_id":1}]"#;
      Mock::given(method("GET"))
        .and(path("/v1/universe/races/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let races = esi.races().list().await.unwrap();

      assert_eq!(races.len(), 1);
      assert_eq!(races[0].name, "Caldari");
    }
  }
}
