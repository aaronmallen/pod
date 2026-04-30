//! Client for EVE killmail ESI endpoints.

use crate::{Error, models::killmail::Killmail};

/// Client for a specific killmail.
pub struct Client<'a> {
  esi: &'a crate::Client,
  hash: String,
  id: i64,
}

impl<'a> Client<'a> {
  /// Creates a new `Client` for the killmail with the given `id` and `hash`.
  pub(crate) fn new(esi: &'a crate::Client, id: i64, hash: &str) -> Self {
    Self {
      esi,
      hash: hash.to_owned(),
      id,
    }
  }

  /// Returns the killmail details.
  pub async fn detail(&self) -> Result<Killmail, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/killmails/{}/{}/", self.id, self.hash))
          .build(),
        None,
      )
      .await
  }
}
