//! Universe name/ID resolution endpoints.

use crate::{
  Error,
  clients::universe::Client,
  models::universe::{ResolvedIds, ResolvedName},
};

impl Client<'_> {
  /// Resolves a list of names to their IDs and categories.
  pub async fn ids(&self, names: &[&str]) -> Result<ResolvedIds, Error> {
    self
      .esi
      .http()
      .post_json_anon(
        &self.esi.url_builder().path("v1/universe/ids/".to_string()).build(),
        &names,
      )
      .await
  }

  /// Resolves a list of IDs to names and categories.
  pub async fn names(&self, ids: &[i64]) -> Result<Vec<ResolvedName>, Error> {
    self
      .esi
      .http()
      .post_json_anon(
        &self.esi.url_builder().path("v3/universe/names/".to_string()).build(),
        &ids,
      )
      .await
  }
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  fn make_esi(server_uri: &str) -> crate::Client {
    crate::Client::builder("test-client")
      .base_url(server_uri)
      .build()
      .unwrap()
  }

  mod ids {
    use super::*;

    #[tokio::test]
    async fn it_resolves_names_to_ids() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/v1/universe/ids/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "characters": [{"id": 90000001i64, "name": "Test Pilot"}],
          "corporations": null,
          "alliances": null,
          "factions": null,
          "inventory_types": null,
          "regions": null,
          "constellations": null,
          "solar_systems": null,
          "stations": null,
          "agents": null
        })))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.universe().ids(&["Test Pilot"]).await.unwrap();

      assert!(result.characters.is_some());
    }

    #[tokio::test]
    async fn it_returns_error_on_500() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/v1/universe/ids/"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({"error": "Internal server error"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.universe().ids(&["Test Pilot"]).await;

      assert!(result.is_err());
    }
  }

  mod names {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_resolves_ids_to_names() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/v3/universe/names/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          {"category": "character", "id": 90000001i64, "name": "Test Pilot"}
        ])))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.universe().names(&[90_000_001i64]).await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].id, 90_000_001i64);
      assert_eq!(result[0].name, "Test Pilot");
      assert_eq!(result[0].category, "character");
    }

    #[tokio::test]
    async fn it_returns_error_on_404() {
      let server = MockServer::start().await;
      Mock::given(method("POST"))
        .and(path("/v3/universe/names/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"error": "Not found"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.universe().names(&[99999999i64]).await;

      assert!(result.is_err());
    }
  }
}
