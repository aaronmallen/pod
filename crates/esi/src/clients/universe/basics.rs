//! Universe ancestry, bloodline, race, faction, graphic, and schematic endpoints.

use crate::{
  Error,
  clients::universe::Client,
  models::universe::{Ancestry, Bloodline, Faction, Graphic, Race, Schematic},
};

impl Client<'_> {
  /// Returns all ancestries.
  pub async fn ancestries(&self) -> Result<Vec<Ancestry>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path("v1/universe/ancestries/".to_string())
          .build(),
        None,
      )
      .await
  }

  /// Returns all bloodlines.
  pub async fn bloodlines(&self) -> Result<Vec<Bloodline>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path("v1/universe/bloodlines/".to_string())
          .build(),
        None,
      )
      .await
  }

  /// Returns all NPC factions.
  pub async fn factions(&self) -> Result<Vec<Faction>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self.esi.url_builder().path("v2/universe/factions/".to_string()).build(),
        None,
      )
      .await
  }

  /// Returns information for a specific graphic.
  pub async fn graphic(&self, graphic_id: i32) -> Result<Graphic, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/universe/graphics/{graphic_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns the IDs of all published graphics.
  pub async fn graphics(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self.esi.url_builder().path("v1/universe/graphics/".to_string()).build(),
        None,
      )
      .await
  }

  /// Returns all playable races.
  pub async fn races(&self) -> Result<Vec<Race>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self.esi.url_builder().path("v1/universe/races/".to_string()).build(),
        None,
      )
      .await
  }

  /// Returns information for a specific planetary industry schematic.
  pub async fn schematic(&self, schematic_id: i32) -> Result<Schematic, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/universe/schematics/{schematic_id}/"))
          .build(),
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

  mod ancestries {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_ancestries() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/universe/ancestries/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          {
            "bloodline_id": 1,
            "description": "A proud ancestry",
            "icon_id": null,
            "id": 42,
            "name": "Gallente Rogue",
            "short_description": null
          }
        ])))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.universe().ancestries().await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].id, 42);
      assert_eq!(result[0].name, "Gallente Rogue");
      assert_eq!(result[0].bloodline_id, 1);
    }

    #[tokio::test]
    async fn it_returns_error_on_404() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/universe/ancestries/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"error": "Not found"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.universe().ancestries().await;

      assert!(result.is_err());
    }
  }
}
