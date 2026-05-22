//! Character asset and blueprint endpoints.

use crate::{
  Error,
  clients::character::AuthenticatedClient,
  models::character::{Asset, AssetLocation, AssetName, Blueprint},
};

impl AuthenticatedClient<'_> {
  /// Returns all assets owned by this character (paginated).
  pub async fn assets(&self) -> Result<Vec<Asset>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v5/characters/{}/assets/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the locations of the given asset item IDs.
  pub async fn asset_locations(&self, item_ids: &[i64]) -> Result<Vec<AssetLocation>, Error> {
    self
      .esi
      .http()
      .post_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/assets/locations/", self.id))
          .build(),
        &item_ids,
        self.grant.access_token(),
      )
      .await
  }

  /// Returns the names of the given asset item IDs.
  pub async fn asset_names(&self, item_ids: &[i64]) -> Result<Vec<AssetName>, Error> {
    self
      .esi
      .http()
      .post_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/assets/names/", self.id))
          .build(),
        &item_ids,
        self.grant.access_token(),
      )
      .await
  }

  /// Returns all blueprints owned by this character (paginated).
  pub async fn blueprints(&self) -> Result<Vec<Blueprint>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v3/characters/{}/blueprints/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}

#[cfg(test)]
mod tests {
  fn make_esi(server_uri: &str) -> (crate::Client, crate::models::auth::Grant) {
    let esi = crate::Client::builder("test-client")
      .base_url(server_uri)
      .build()
      .unwrap();
    let grant = crate::models::auth::Grant::new(
      "test-token",
      90_000_001i64,
      "Test Char",
      std::time::SystemTime::now() + std::time::Duration::from_secs(3600),
      "refresh",
      vec![],
    );
    (esi, grant)
  }

  mod assets {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;

    #[tokio::test]
    async fn it_returns_assets_on_success() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v5/characters/90000001/assets/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_raw(
              r#"[{"is_singleton":false,"item_id":1001,"location_flag":"Hangar","location_id":60003760,"location_type":"station","quantity":1,"type_id":587}]"#,
              "application/json",
            ),
        )
        .mount(&server)
        .await;
      let (esi, grant) = make_esi(&server.uri());
      let auth = esi.character(&grant);

      let result = auth.assets().await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].item_id, 1001);
      assert_eq!(result[0].type_id, 587);
      assert_eq!(result[0].location_flag, "Hangar");
      assert_eq!(result[0].location_id, 60003760);
      assert_eq!(result[0].location_type, "station");
      assert_eq!(result[0].quantity, 1);
      assert!(!result[0].is_singleton);
    }

    #[tokio::test]
    async fn it_returns_api_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v5/characters/90000001/assets/"))
        .respond_with(ResponseTemplate::new(401).set_body_raw(r#"{"error":"Unauthorized"}"#, "application/json"))
        .mount(&server)
        .await;
      let (esi, grant) = make_esi(&server.uri());
      let auth = esi.character(&grant);

      let result = auth.assets().await;

      assert!(matches!(
        result,
        Err(crate::Error::Api {
          status: 401,
          ..
        })
      ));
    }

    #[tokio::test]
    async fn it_returns_api_error_on_500() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v5/characters/90000001/assets/"))
        .respond_with(
          ResponseTemplate::new(500).set_body_raw(r#"{"error":"Internal Server Error"}"#, "application/json"),
        )
        .mount(&server)
        .await;
      let (esi, grant) = make_esi(&server.uri());
      let auth = esi.character(&grant);

      let result = auth.assets().await;

      assert!(matches!(
        result,
        Err(crate::Error::Api {
          status: 500,
          ..
        })
      ));
    }
  }
}
