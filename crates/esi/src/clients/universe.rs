//! Client for EVE universe ESI endpoints.

pub mod basics;
pub mod geography;
pub mod items;
pub mod resolve;

use crate::{Client as EsiClient, models::auth::Grant};

/// Client for universe ESI endpoints.
pub struct Client<'a> {
  pub(in crate::clients::universe) esi: &'a EsiClient,
}

impl<'a> Client<'a> {
  /// Creates a new `Client`.
  pub(crate) fn new(esi: &'a EsiClient) -> Self {
    Self {
      esi,
    }
  }

  /// Returns a [`UniverseStructureClient`] for the given structure.
  pub fn structure(&self, id: i64) -> UniverseStructureClient<'_> {
    UniverseStructureClient {
      esi: self.esi,
      id,
    }
  }
}

/// Client for a specific universe structure.
pub struct UniverseStructureClient<'a> {
  pub(in crate::clients::universe) esi: &'a EsiClient,
  pub(in crate::clients::universe) id: i64,
}

impl<'a> UniverseStructureClient<'a> {
  /// Returns an authenticated structure client bound to the given grant.
  pub fn auth<'b>(&'b self, grant: &'b Grant) -> AuthenticatedUniverseStructureClient<'b> {
    AuthenticatedUniverseStructureClient {
      esi: self.esi,
      grant,
      structure_id: self.id,
    }
  }
}

/// Authenticated client for universe structure endpoints requiring a valid OAuth2 grant.
pub struct AuthenticatedUniverseStructureClient<'a> {
  pub(in crate::clients::universe) esi: &'a EsiClient,
  pub(in crate::clients::universe) grant: &'a Grant,
  pub(in crate::clients::universe) structure_id: i64,
}
