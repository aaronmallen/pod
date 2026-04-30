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
