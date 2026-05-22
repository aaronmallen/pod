//! Universe geography endpoints — systems, regions, planets, etc.

use crate::{
  Error,
  clients::universe::{AuthenticatedUniverseStructureClient, Client},
  models::universe::{
    AsteroidBelt, Constellation, Moon, Planet, Region, SolarSystem, Star, Stargate, Station, SystemJump, SystemKill,
    UniverseStructure,
  },
};

impl Client<'_> {
  /// Returns information for a specific asteroid belt.
  pub async fn asteroid_belt(&self, asteroid_belt_id: i64) -> Result<AsteroidBelt, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/universe/asteroid_belts/{asteroid_belt_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns information for a specific constellation.
  pub async fn constellation(&self, constellation_id: i64) -> Result<Constellation, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/universe/constellations/{constellation_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns the IDs of all published constellations.
  pub async fn constellations(&self) -> Result<Vec<i64>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path("v1/universe/constellations/".to_string())
          .build(),
        None,
      )
      .await
  }

  /// Returns information for a specific moon.
  pub async fn moon(&self, moon_id: i64) -> Result<Moon, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/universe/moons/{moon_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns information for a specific planet.
  pub async fn planet(&self, planet_id: i64) -> Result<Planet, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/universe/planets/{planet_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns information for a specific region.
  pub async fn region(&self, region_id: i64) -> Result<Region, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/universe/regions/{region_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns the IDs of all published regions.
  pub async fn regions(&self) -> Result<Vec<i64>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self.esi.url_builder().path("v1/universe/regions/".to_string()).build(),
        None,
      )
      .await
  }

  /// Returns information for a specific solar system.
  pub async fn solar_system(&self, system_id: i64) -> Result<SolarSystem, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v4/universe/systems/{system_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns the IDs of all published solar systems.
  pub async fn solar_systems(&self) -> Result<Vec<i64>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self.esi.url_builder().path("v1/universe/systems/".to_string()).build(),
        None,
      )
      .await
  }

  /// Returns information for a specific star.
  pub async fn star(&self, star_id: i64) -> Result<Star, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/universe/stars/{star_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns information for a specific stargate.
  pub async fn stargate(&self, stargate_id: i64) -> Result<Stargate, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/universe/stargates/{stargate_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns information for a specific station.
  pub async fn station(&self, station_id: i64) -> Result<Station, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/universe/stations/{station_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns the IDs of all published stations.
  pub async fn stations(&self) -> Result<Vec<i64>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self.esi.url_builder().path("v1/universe/stations/".to_string()).build(),
        None,
      )
      .await
  }

  /// Returns jump counts for all systems that had at least one jump in the past hour.
  pub async fn system_jumps(&self) -> Result<Vec<SystemJump>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path("v1/universe/system_jumps/".to_string())
          .build(),
        None,
      )
      .await
  }

  /// Returns kill counts for all systems that had at least one kill in the past hour.
  pub async fn system_kills(&self) -> Result<Vec<SystemKill>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path("v2/universe/system_kills/".to_string())
          .build(),
        None,
      )
      .await
  }
}

impl AuthenticatedUniverseStructureClient<'_> {
  /// Returns information about this player-owned structure.
  pub async fn detail(&self) -> Result<UniverseStructure, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/universe/structures/{}/", self.structure_id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the IDs of all structures that the character has access to.
  pub async fn structures(&self) -> Result<Vec<i64>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path("v1/universe/structures/".to_string())
          .build(),
        Some(self.grant.access_token()),
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

  mod region {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_region() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/universe/regions/10000002/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "constellations": [20000020i64, 20000021i64],
          "description": null,
          "name": "The Forge",
          "region_id": 10000002i64
        })))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.universe().region(10_000_002i64).await.unwrap();

      assert_eq!(result.name, "The Forge");
      assert_eq!(result.region_id, 10_000_002i64);
      assert_eq!(result.constellations.len(), 2);
    }

    #[tokio::test]
    async fn it_returns_error_on_404() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/universe/regions/10000002/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"error": "Not found"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.universe().region(10_000_002i64).await;

      assert!(result.is_err());
    }
  }
}
