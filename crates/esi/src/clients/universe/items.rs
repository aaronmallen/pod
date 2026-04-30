//! Universe item category, group, and type endpoints.

use crate::{
  Error,
  clients::universe::Client,
  models::universe::{Category, Group, TypeInfo},
};

impl Client<'_> {
  /// Returns information for a specific item category.
  pub async fn category(&self, category_id: i32) -> Result<Category, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/universe/categories/{category_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns the IDs of all published item categories.
  pub async fn categories(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path("v1/universe/categories/".to_string())
          .build(),
        None,
      )
      .await
  }

  /// Returns information for a specific item group.
  pub async fn group(&self, group_id: i32) -> Result<Group, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/universe/groups/{group_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns the IDs of all published item groups (paginated).
  pub async fn groups(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self.esi.url_builder().path("v1/universe/groups/".to_string()).build(),
        None,
      )
      .await
  }

  /// Returns information for a specific item type.
  pub async fn type_info(&self, type_id: i32) -> Result<TypeInfo, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v3/universe/types/{type_id}/"))
          .build(),
        None,
      )
      .await
  }

  /// Returns the IDs of all published item types (paginated).
  pub async fn types(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self.esi.url_builder().path("v1/universe/types/".to_string()).build(),
        None,
      )
      .await
  }
}
