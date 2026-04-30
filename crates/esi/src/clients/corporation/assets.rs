//! Corporation asset and blueprint endpoints.

use crate::{
  Error,
  clients::corporation::AuthenticatedClient,
  models::character::{Asset, AssetLocation, AssetName, Blueprint},
};

impl AuthenticatedClient<'_> {
  /// Returns all assets owned by this corporation (paginated).
  pub async fn assets(&self) -> Result<Vec<Asset>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v5/corporations/{}/assets/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the locations of the given asset item IDs.
  pub async fn asset_locations(&self, item_ids: &[i64]) -> Result<Vec<AssetLocation>, Error> {
    self
      .esi
      .http()
      .post_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/assets/locations/", self.id))
          .build(),
        &item_ids,
        self.grant.access_token(),
      )
      .await
  }

  /// Returns the names of the given asset item IDs.
  pub async fn asset_names(&self, item_ids: &[i64]) -> Result<Vec<AssetName>, Error> {
    self
      .esi
      .http()
      .post_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/assets/names/", self.id))
          .build(),
        &item_ids,
        self.grant.access_token(),
      )
      .await
  }

  /// Returns all blueprints owned by this corporation (paginated).
  pub async fn blueprints(&self) -> Result<Vec<Blueprint>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v3/corporations/{}/blueprints/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}
