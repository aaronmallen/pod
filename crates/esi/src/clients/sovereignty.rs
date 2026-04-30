//! Client for EVE sovereignty ESI endpoints.

use crate::{
  Error,
  models::sovereignty::{SovereigntyCampaign, SovereigntyMap, SovereigntyStructure},
};

/// Client for sovereignty ESI endpoints.
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

  /// Returns all active sovereignty campaigns.
  pub async fn campaigns(&self) -> Result<Vec<SovereigntyCampaign>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/sovereignty/campaigns/").build(), None)
      .await
  }

  /// Returns sovereignty data for all solar systems.
  pub async fn map(&self) -> Result<Vec<SovereigntyMap>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/sovereignty/map/").build(), None)
      .await
  }

  /// Returns all sovereignty structures.
  pub async fn structures(&self) -> Result<Vec<SovereigntyStructure>, Error> {
    self
      .esi
      .http()
      .get_json(&self.esi.url_builder().path("v1/sovereignty/structures/").build(), None)
      .await
  }
}
