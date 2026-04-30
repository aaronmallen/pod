//! Client for EVE alliance ESI endpoints.

use crate::{
  Error,
  models::{
    alliance::{AllianceContact, AllianceContactLabel, AllianceDetail, AllianceIcons},
    auth::Grant,
  },
};

/// Client for alliance-scoped ESI endpoints.
pub struct Client<'a> {
  esi: &'a crate::Client,
  id: i64,
}

impl<'a> Client<'a> {
  /// Creates a new `Client` for the alliance with the given `id`.
  pub(crate) fn new(esi: &'a crate::Client, id: i64) -> Self {
    Self {
      esi,
      id,
    }
  }

  /// Returns an authenticated alliance client bound to the given grant.
  pub fn auth(&self, grant: &'a Grant) -> AuthenticatedClient<'a> {
    AuthenticatedClient {
      esi: self.esi,
      grant,
      id: self.id,
    }
  }

  /// Returns the IDs of corporations in this alliance.
  pub async fn corporation_ids(&self) -> Result<Vec<i64>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/alliances/{}/corporations/", self.id))
          .build(),
        None,
      )
      .await
  }

  /// Returns public information for this alliance.
  pub async fn detail(&self) -> Result<AllianceDetail, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v3/alliances/{}/", self.id))
          .build(),
        None,
      )
      .await
  }

  /// Returns the icon URLs for this alliance.
  pub async fn icons(&self) -> Result<AllianceIcons, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/alliances/{}/icons/", self.id))
          .build(),
        None,
      )
      .await
  }
}

/// Authenticated client for alliance-scoped ESI endpoints requiring a valid OAuth2 grant.
pub struct AuthenticatedClient<'a> {
  esi: &'a crate::Client,
  grant: &'a Grant,
  id: i64,
}

impl<'a> AuthenticatedClient<'a> {
  /// Returns the alliance contact labels.
  pub async fn contact_labels(&self) -> Result<Vec<AllianceContactLabel>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/alliances/{}/contacts/labels/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the alliance contact list (paginated).
  pub async fn contacts(&self) -> Result<Vec<AllianceContact>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/alliances/{}/contacts/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}
