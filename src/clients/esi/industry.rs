use crate::clients::{
  self,
  esi::{Client as EsiClient, models::industry::SystemCostIndices},
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

  pub async fn system_cost_indices(&self) -> Result<Vec<SystemCostIndices>, clients::Error> {
    let url = self.esi.url("industry/systems/");
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

  mod system_cost_indices {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_every_system_with_its_per_activity_indices() {
      let server = MockServer::start().await;
      let body = r#"[{"solar_system_id":30000142,"cost_indices":[{"activity":"manufacturing","cost_index":0.05},{"activity":"reaction","cost_index":0.01}]},{"solar_system_id":30002187,"cost_indices":[{"activity":"copying","cost_index":0.02}]}]"#;
      Mock::given(method("GET"))
        .and(path("/industry/systems/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let systems = esi.industry().system_cost_indices().await.unwrap();

      assert_eq!(systems.len(), 2);
      assert_eq!(systems[0].solar_system_id, 30_000_142);
      assert_eq!(systems[0].cost_indices.len(), 2);
      assert_eq!(systems[0].cost_indices[0].activity, "manufacturing");
      assert_eq!(systems[0].cost_indices[0].cost_index, 0.05);
      assert_eq!(systems[1].solar_system_id, 30_002_187);
      assert_eq!(systems[1].cost_indices[0].activity, "copying");
    }

    #[tokio::test]
    async fn it_returns_http_error_on_5xx() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/industry/systems/"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
      let esi = make_esi(&server.uri()).await;

      let result = esi.industry().system_cost_indices().await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }
  }
}
