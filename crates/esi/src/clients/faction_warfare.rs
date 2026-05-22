//! Client for EVE faction warfare ESI endpoints.

use crate::{
  Error,
  models::faction_warfare::{
    FwCharacterLeaderboard, FwCorporationLeaderboard, FwLeaderboard, FwStats, FwSystem, FwWar,
  },
};

/// Client for faction warfare ESI endpoints.
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

  /// Returns faction warfare leaderboards for characters.
  pub async fn character_leaderboards(&self) -> Result<FwCharacterLeaderboard, Error> {
    self
      .esi
      .http()
      .get_json(
        &self.esi.url_builder().path("v1/fw/leaderboards/characters/").build(),
        None,
      )
      .await
  }

  /// Returns faction warfare leaderboards for corporations.
  pub async fn corporation_leaderboards(&self) -> Result<FwCorporationLeaderboard, Error> {
    self
      .esi
      .http()
      .get_json(
        &self.esi.url_builder().path("v1/fw/leaderboards/corporations/").build(),
        None,
      )
      .await
  }

  /// Returns faction warfare leaderboards for factions.
  pub async fn leaderboards(&self) -> Result<FwLeaderboard, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/fw/leaderboards/").build(), None)
      .await
  }

  /// Returns faction warfare statistics for each faction.
  pub async fn stats(&self) -> Result<Vec<FwStats>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/fw/stats/").build(), None)
      .await
  }

  /// Returns the current faction warfare solar systems.
  pub async fn systems(&self) -> Result<Vec<FwSystem>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/fw/systems/").build(), None)
      .await
  }

  /// Returns active faction warfare matchups.
  pub async fn wars(&self) -> Result<Vec<FwWar>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/fw/wars/").build(), None)
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

  mod systems {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_fw_systems() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/fw/systems/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          {
            "contested": "uncontested",
            "occupier_faction_id": 500001i64,
            "owner_faction_id": 500001i64,
            "solar_system_id": 30002187i64,
            "victory_points": 0,
            "victory_points_threshold": 3000
          }
        ])))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.faction_warfare().systems().await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].solar_system_id, 30_002_187i64);
      assert_eq!(result[0].contested, "uncontested");
      assert_eq!(result[0].occupier_faction_id, 500_001i64);
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/fw/systems/"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({"error": "Internal server error"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.faction_warfare().systems().await;

      assert!(result.is_err());
    }
  }

  mod wars {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_fw_wars() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/fw/wars/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          {"aggressor_faction_id": 500001i64, "defender_faction_id": 500002i64}
        ])))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.faction_warfare().wars().await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].aggressor_faction_id, 500_001i64);
      assert_eq!(result[0].defender_faction_id, 500_002i64);
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/fw/wars/"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({"error": "Internal server error"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.faction_warfare().wars().await;

      assert!(result.is_err());
    }
  }
}
