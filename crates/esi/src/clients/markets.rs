//! Client for EVE market order ESI endpoints (unauthenticated, region-scoped).

pub mod orders;

use crate::Client as EsiClient;

/// Client for market order ESI endpoints.
pub struct Client<'a> {
  pub(super) esi: &'a EsiClient,
}

impl<'a> Client<'a> {
  /// Creates a new `Client`.
  pub(crate) fn new(esi: &'a EsiClient) -> Self {
    Self {
      esi,
    }
  }
}
