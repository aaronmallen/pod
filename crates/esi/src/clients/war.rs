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
