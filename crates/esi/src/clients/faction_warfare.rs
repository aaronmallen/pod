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
