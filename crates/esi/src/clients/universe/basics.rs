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
