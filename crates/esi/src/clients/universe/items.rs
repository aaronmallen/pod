//! Universe item category, group, and type endpoints.

use crate::{
  Error,
  clients::universe::Client,
  models::universe::{Category, Group, TypeInfo},
};

impl Client<'_> {
  /// Returns information for a specific item category.
  pub async fn category(&self, category_id: i32) -> Result<Category, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/universe/categories/{category_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns the IDs of all published item categories.
  pub async fn categories(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path("v1/universe/categories/".to_string())
          .build(),
        None,
      )
      .await
  }

  /// Returns information for a specific item group.
  pub async fn group(&self, group_id: i32) -> Result<Group, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/universe/groups/{group_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns the IDs of all published item groups (paginated).
  pub async fn groups(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self.esi.url_builder().path("v1/universe/groups/".to_string()).build(),
        None,
      )
      .await
  }

  /// Returns information for a specific item type.
  pub async fn type_info(&self, type_id: i32) -> Result<TypeInfo, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v3/universe/types/{type_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns the IDs of all published item types (paginated).
  pub async fn types(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self.esi.url_builder().path("v1/universe/types/".to_string()).build(),
        None,
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

  mod category {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_category() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/universe/categories/6/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "category_id": 6,
          "groups": [25, 26, 27],
          "name": "Ship",
          "published": true
        })))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.universe().category(6).await.unwrap();

      assert_eq!(result.category_id, 6);
      assert_eq!(result.name, "Ship");
      assert!(result.published);
      assert_eq!(result.groups.len(), 3);
    }

    #[tokio::test]
    async fn it_returns_error_on_404() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/universe/categories/6/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"error": "Not found"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.universe().category(6).await;

      assert!(result.is_err());
    }
  }
}
