use crate::clients::{
  self,
  esi::{Client as EsiClient, models::bloodlines::Bloodline},
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

  pub async fn list(&self) -> Result<Vec<Bloodline>, clients::Error> {
    let url = self.esi.url("universe/bloodlines/");
    self.esi.get_json(&url, None).await
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
    async fn it_returns_bloodlines() {
      let server = MockServer::start().await;
      let body = r#"[{"bloodline_id":1,"charisma":6,"corporation_id":1000006,"description":"The Deteis.","intelligence":7,"memory":7,"name":"Deteis","perception":5,"race_id":1,"ship_type_id":601,"willpower":5}]"#;
      Mock::given(method("GET"))
        .and(path("/universe/bloodlines/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let bloodlines = esi.bloodlines().list().await.unwrap();

      assert_eq!(bloodlines.len(), 1);
      assert_eq!(bloodlines[0].name, "Deteis");
    }
  }
}
