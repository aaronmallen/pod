//! Client for EVE industry ESI endpoints.

use crate::{
  Error,
  models::industry::{IndustryFacility, IndustrySolarSystem},
};

/// Client for industry ESI endpoints.
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

  /// Returns all industry facilities.
  pub async fn facilities(&self) -> Result<Vec<IndustryFacility>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/industry/facilities/").build(), None)
      .await
  }

  /// Returns solar system industry cost indices.
  pub async fn systems(&self) -> Result<Vec<IndustrySolarSystem>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/industry/systems/").build(), None)
      .await
  }
}
