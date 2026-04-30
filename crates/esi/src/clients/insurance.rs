//! Client for EVE insurance ESI endpoints.

use crate::{Error, models::insurance::InsurancePrice};

/// Client for insurance ESI endpoints.
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

  /// Returns insurance prices for all ship types.
  pub async fn prices(&self) -> Result<Vec<InsurancePrice>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/insurance/prices/").build(), None)
      .await
  }
}
