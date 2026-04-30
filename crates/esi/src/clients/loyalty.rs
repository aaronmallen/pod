//! Client for EVE loyalty store ESI endpoints.

use crate::{Error, models::loyalty::LoyaltyOffer};

/// Client for a specific NPC corporation loyalty store.
pub struct Client<'a> {
  corp_id: i64,
  esi: &'a crate::Client,
}

impl<'a> Client<'a> {
  /// Creates a new `Client` for the NPC corporation with the given `corp_id`.
  pub(crate) fn new(esi: &'a crate::Client, corp_id: i64) -> Self {
    Self {
      corp_id,
      esi,
    }
  }

  /// Returns the loyalty store offers for this NPC corporation.
  pub async fn offers(&self) -> Result<Vec<LoyaltyOffer>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/loyalty/stores/{}/offers/", self.corp_id))
          .build(),
        None,
      )
      .await
  }
}
