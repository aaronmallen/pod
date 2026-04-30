//! Client for EVE corporation ESI endpoints.

pub mod assets;
pub mod contacts;
pub mod contracts;
pub mod industry;
pub mod members;
pub mod misc;
pub mod wallets;

use crate::{
  Client as EsiClient, Error,
  models::{
    auth::Grant,
    corporation::{AllianceHistoryEntry, CorporationDetail, CorporationIcons},
  },
};

/// Client for corporation ESI endpoints.
pub struct Client<'a> {
  pub(in crate::clients::corporation) esi: &'a EsiClient,
  pub(in crate::clients::corporation) id: i64,
}

impl<'a> Client<'a> {
  /// Creates a new `Client` for the corporation with the given `id`.
  pub(crate) fn new(esi: &'a EsiClient, id: i64) -> Self {
    Self {
      esi,
      id,
    }
  }

  /// Returns an authenticated client bound to the given grant.
  pub fn auth<'b>(&'b self, grant: &'b Grant) -> AuthenticatedClient<'b> {
    AuthenticatedClient {
      esi: self.esi,
      grant,
      id: self.id,
    }
  }

  /// Returns the alliance history for this corporation.
  pub async fn alliance_history(&self) -> Result<Vec<AllianceHistoryEntry>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v3/corporations/{}/alliancehistory/", self.id))
          .build(),
        None,
      )
      .await
  }

  /// Returns public information for this corporation.
  pub async fn detail(&self) -> Result<CorporationDetail, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v5/corporations/{}/", self.id))
          .build(),
        None,
      )
      .await
  }

  /// Returns the icon URLs for this corporation.
  pub async fn icons(&self) -> Result<CorporationIcons, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/icons/", self.id))
          .build(),
        None,
      )
      .await
  }
}

/// Authenticated client for corporation ESI endpoints requiring a valid OAuth2 grant.
pub struct AuthenticatedClient<'a> {
  pub(in crate::clients::corporation) esi: &'a EsiClient,
  pub(in crate::clients::corporation) grant: &'a Grant,
  pub(in crate::clients::corporation) id: i64,
}
