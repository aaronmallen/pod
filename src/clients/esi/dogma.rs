use crate::clients::{
  self,
  esi::{Client as EsiClient, models::dogma::DynamicItem},
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

  pub async fn dynamic_item(&self, type_id: i64, item_id: i64) -> Result<DynamicItem, clients::Error> {
    let url = self.esi.url(&format!("dogma/dynamic/items/{type_id}/{item_id}/"));
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

  mod dynamic_item {
    use pretty_assertions::assert_eq;

    use super::*;

    const DYNAMIC_ITEM_FIXTURE: &str = include_str!("../../../test/fixtures/esi/dogma_dynamic_item.json");

    #[tokio::test]
    async fn it_returns_source_mutator_and_rolled_attributes() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/dogma/dynamic/items/47804/1038913810254/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(DYNAMIC_ITEM_FIXTURE, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let item = esi.dogma().dynamic_item(47804, 1_038_913_810_254).await.unwrap();

      assert_eq!(item.source_type_id, 2488);
      assert_eq!(item.mutator_type_id, 49730);
      assert_eq!(item.dogma_attributes.len(), 3);
      assert_eq!(item.dogma_attributes[0].attribute_id, 6);
      assert_eq!(item.dogma_attributes[0].value, 1.2);
    }

    #[tokio::test]
    async fn it_returns_http_error_on_4xx() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/dogma/dynamic/items/1/2/"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let result = esi.dogma().dynamic_item(1, 2).await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }
  }
}
