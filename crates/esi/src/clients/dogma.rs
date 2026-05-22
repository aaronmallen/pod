//! Client for EVE dogma ESI endpoints.

use crate::{
  Error,
  models::dogma::{DogmaAttribute, DogmaEffect, DynamicItem},
};

/// Client for dogma ESI endpoints.
pub struct Client<'a> {
  esi: &'a crate::Client,
}

impl<'a> Client<'a> {
  /// Creates a new `Client`.
  pub(crate) fn new(esi: &'a crate::Client) -> Self {
    Self {
      esi,
    }
  }

  /// Returns the definition for a specific dogma attribute.
  pub async fn attribute(&self, id: i32) -> Result<DogmaAttribute, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/dogma/attributes/{id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns all dogma attribute IDs.
  pub async fn attribute_ids(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/dogma/attributes/").build(), None)
      .await
  }

  /// Returns the dogma attributes and effects for a dynamically mutated item.
  pub async fn dynamic_item(&self, type_id: i64, item_id: i64) -> Result<DynamicItem, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/dogma/dynamic/items/{type_id}/{item_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns the definition for a specific dogma effect.
  pub async fn effect(&self, id: i32) -> Result<DogmaEffect, Error> {
    self
      .esi
      .http()
      .get_json(
        &self.esi.url_builder().path(format!("v2/dogma/effects/{id}/")).build(),
        None,
      )
      .await
  }

  /// Returns all dogma effect IDs.
  pub async fn effect_ids(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/dogma/effects/").build(), None)
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

  mod attribute {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_a_dogma_attribute() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v2/dogma/attributes/20/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "attribute_id": 20,
          "default_value": 1.0,
          "description": "Armor hitpoints.",
          "display_name": "Armor HP",
          "high_is_good": true,
          "icon_id": 1374,
          "name": "armorHP",
          "published": true,
          "stackable": true,
          "unit_id": 105
        })))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let attr = esi.dogma().attribute(20).await.unwrap();

      assert_eq!(attr.attribute_id, 20);
      assert_eq!(attr.name.as_deref(), Some("armorHP"));
      assert_eq!(attr.high_is_good, Some(true));
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v2/dogma/attributes/20/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"error": "Attribute not found"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.dogma().attribute(20).await;

      assert!(result.is_err());
    }
  }
}
