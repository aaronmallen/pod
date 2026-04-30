//! Character clones, implants, and fittings endpoints.

use crate::{
  Error,
  clients::character::AuthenticatedClient,
  models::character::{Clones, Fitting, FittingId, NewFitting},
};

impl AuthenticatedClient<'_> {
  /// Returns clone information for this character.
  pub async fn clones(&self) -> Result<Clones, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v4/characters/{}/clones/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Creates a new fitting and returns its ID.
  pub async fn create_fitting(&self, body: NewFitting) -> Result<FittingId, Error> {
    self
      .esi
      .http()
      .post_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/fittings/", self.id))
          .build(),
        &body,
        self.grant.access_token(),
      )
      .await
  }

  /// Deletes a saved fitting.
  pub async fn delete_fitting(&self, fitting_id: i64) -> Result<(), Error> {
    self
      .esi
      .http()
      .delete_empty(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/fittings/{fitting_id}/", self.id))
          .build(),
        self.grant.access_token(),
      )
      .await
  }

  /// Returns saved fittings for this character.
  pub async fn fittings(&self) -> Result<Vec<Fitting>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/fittings/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns active implants for this character.
  pub async fn implants(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/implants/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}
