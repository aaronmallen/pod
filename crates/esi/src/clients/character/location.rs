//! Character location, fleet, loyalty, killmail, and search endpoints.

use crate::{
  Error,
  clients::character::AuthenticatedClient,
  models::character::{
    CharacterFleet, CharacterLocation, CharacterOnline, CharacterShip, LoyaltyPoint, RecentKillmail, SearchResults,
  },
};

impl AuthenticatedClient<'_> {
  /// Returns the fleet info for this character, if they are in a fleet.
  pub async fn fleet(&self) -> Result<CharacterFleet, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/fleet/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the current location of this character.
  pub async fn location(&self) -> Result<CharacterLocation, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/location/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the loyalty point balances for this character.
  pub async fn loyalty_points(&self) -> Result<Vec<LoyaltyPoint>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/loyalty/points/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns recent killmails for this character (paginated).
  pub async fn killmails(&self) -> Result<Vec<RecentKillmail>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/killmails/recent/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the online status of this character.
  pub async fn online(&self) -> Result<CharacterOnline, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v3/characters/{}/online/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Searches for entities matching the given query string.
  pub async fn search(&self, query: &str, categories: &[&str]) -> Result<SearchResults, Error> {
    let categories_str = categories.join(",");
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v3/characters/{}/search/", self.id))
          .param("categories", categories_str)
          .param("search", query)
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the current ship this character is flying.
  pub async fn ship(&self) -> Result<CharacterShip, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/ship/", self.id))
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
      90_000_001i64,
      "Test Char",
      SystemTime::now() + Duration::from_secs(3600),
      "refresh",
      vec![],
    )
  }

  mod location {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_location_on_success() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v2/characters/90000001/location/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"solar_system_id": 30000142, "station_id": 60003760}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.location().await.unwrap();

      assert_eq!(result.solar_system_id, 30000142);
      assert_eq!(result.station_id, Some(60003760));
    }

    #[tokio::test]
    async fn it_returns_api_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v2/characters/90000001/location/"))
        .respond_with(ResponseTemplate::new(401).set_body_raw(r#"{"error": "Unauthorized"}"#, "application/json"))
        .mount(&server)
        .await;
      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.location().await;

      assert!(matches!(
        result,
        Err(crate::Error::Api {
          status: 401,
          ..
        })
      ));
    }
  }
}
