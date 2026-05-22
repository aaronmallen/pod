//! Corporation asset and blueprint endpoints.

use crate::{
  Error,
  clients::corporation::AuthenticatedClient,
  models::character::{Asset, AssetLocation, AssetName, Blueprint},
};

impl AuthenticatedClient<'_> {
  /// Returns all assets owned by this corporation (paginated).
  pub async fn assets(&self) -> Result<Vec<Asset>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v5/corporations/{}/assets/", self.id))
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
          .path(format!("v2/corporations/{}/assets/locations/", self.id))
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
          .path(format!("v1/corporations/{}/assets/names/", self.id))
          .build(),
        &item_ids,
        self.grant.access_token(),
      )
      .await
  }

  /// Returns all blueprints owned by this corporation (paginated).
  pub async fn blueprints(&self) -> Result<Vec<Blueprint>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v3/corporations/{}/blueprints/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}

#[cfg(test)]
mod tests {
  use std::time::{Duration, SystemTime};

  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  fn make_grant() -> crate::models::auth::Grant {
    crate::models::auth::Grant::new(
      "test-token",
      123_456_789i64,
      "Test Member",
      SystemTime::now() + Duration::from_secs(3600),
      "refresh",
      vec![],
    )
  }

  mod assets {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_corporation_assets() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v5/corporations/109299958/assets/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([
              {
                "is_blueprint_copy": null,
                "is_singleton": false,
                "item_id": 1_000_000_001i64,
                "location_flag": "Hangar",
                "location_id": 60_002_959i64,
                "location_type": "station",
                "quantity": 5,
                "type_id": 34
              }
            ])),
        )
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let corp = esi.corporation(109_299_958i64);
      let auth = corp.auth(&grant);

      let assets = auth.assets().await.unwrap();

      assert_eq!(assets.len(), 1);
      assert_eq!(assets[0].item_id, 1_000_000_001i64);
      assert_eq!(assets[0].type_id, 34);
      assert_eq!(assets[0].quantity, 5);
    }

    #[tokio::test]
    async fn it_returns_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v5/corporations/109299958/assets/"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({"error": "Unauthorized"})))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let corp = esi.corporation(109_299_958i64);
      let auth = corp.auth(&grant);

      let result = auth.assets().await;

      assert!(result.is_err());
    }
  }
}
