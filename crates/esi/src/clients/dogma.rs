//! Client for EVE dogma ESI endpoints.

use crate::{
  Error,
  models::dogma::{DogmaAttribute, DogmaEffect, DynamicItem},
};

/// Client for dogma ESI endpoints.
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

  /// Returns the definition for a specific dogma attribute.
  pub async fn attribute(&self, id: i32) -> Result<DogmaAttribute, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/dogma/attributes/{id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns all dogma attribute IDs.
  pub async fn attribute_ids(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/dogma/attributes/").build(), None)
      .await
  }

  /// Returns the dogma attributes and effects for a dynamically mutated item.
  pub async fn dynamic_item(&self, type_id: i64, item_id: i64) -> Result<DynamicItem, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/dogma/dynamic/items/{type_id}/{item_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns the definition for a specific dogma effect.
  pub async fn effect(&self, id: i32) -> Result<DogmaEffect, Error> {
    self
      .esi
      .http()
      .get_json(
        &self.esi.url_builder().path(format!("v2/dogma/effects/{id}/")).build(),
        None,
      )
      .await
  }

  /// Returns all dogma effect IDs.
  pub async fn effect_ids(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/dogma/effects/").build(), None)
      .await
  }
}
