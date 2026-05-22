//! Client for EVE war ESI endpoints.

use crate::{
  Error,
  models::war::{War, WarKillmail},
};

/// Client for a specific war.
pub struct Client<'a> {
  esi: &'a crate::Client,
  id: i32,
}

impl<'a> Client<'a> {
  /// Creates a new `Client` for the war with the given `id`.
  pub(crate) fn new(esi: &'a crate::Client, id: i64) -> Self {
    Self {
      esi,
      id: id as i32,
    }
  }

  /// Returns details for this war.
  pub async fn detail(&self) -> Result<War, Error> {
    self
      .esi
      .http()
      .get_json(
        &self.esi.url_builder().path(format!("v1/wars/{}/", self.id)).build(),
        None,
      )
      .await
  }

  /// Returns all killmails for this war (paginated).
  pub async fn killmails(&self) -> Result<Vec<WarKillmail>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/wars/{}/killmails/", self.id))
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

  mod detail {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_war_detail() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/wars/1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "aggressor": {
            "alliance_id": 99_000_001,
            "corporations_taken": 0,
            "isk_destroyed": 1_000_000.0,
            "ships_killed": 5
          },
          "allies": null,
          "declared": "2023-01-01T00:00:00Z",
          "defender": {
            "alliance_id": 99_000_002,
            "corporations_taken": 0,
            "isk_destroyed": 0.0,
            "ships_killed": 0
          },
          "finished": null,
          "id": 1,
          "mutual": false,
          "open_for_allies": true,
          "retracted": null,
          "started": "2023-01-02T00:00:00Z"
        })))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let war = esi.war(1).detail().await.unwrap();

      assert_eq!(war.id, 1);
      assert_eq!(war.mutual, false);
      assert_eq!(war.open_for_allies, true);
      assert_eq!(war.aggressor.ships_killed, 5);
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/wars/999/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"error": "War not found"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.war(999).detail().await;

      assert!(result.is_err());
    }
  }
}
