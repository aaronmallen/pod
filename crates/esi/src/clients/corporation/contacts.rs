//! Corporation contact endpoints.

use crate::{
  Error,
  clients::corporation::AuthenticatedClient,
  models::corporation::{CorporationContact, CorporationContactLabel},
};

impl AuthenticatedClient<'_> {
  /// Returns all contacts for this corporation (paginated).
  pub async fn contacts(&self) -> Result<Vec<CorporationContact>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/contacts/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns all contact labels for this corporation.
  pub async fn contact_labels(&self) -> Result<Vec<CorporationContactLabel>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/contacts/labels/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}
