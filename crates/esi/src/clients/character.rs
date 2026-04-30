//! Client for EVE character ESI endpoints.

pub mod assets;
pub mod calendar;
pub mod clones;
pub mod contacts;
pub mod contracts;
pub mod industry;
pub mod location;
pub mod mail;
pub mod search;
pub mod skills;
pub mod wallet;

use crate::{
  Client as EsiClient, Error,
  models::{
    auth::Grant,
    character::{CharacterDetail, CharacterPortrait, CorporationHistoryEntry},
  },
};

/// Client for character ESI endpoints.
pub struct Client<'a> {
  pub(in crate::clients::character) esi: &'a EsiClient,
  pub(in crate::clients::character) id: i64,
}

impl<'a> Client<'a> {
  /// Creates a new `Client` for the character with the given `id`.
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

  /// Returns the corporation history for this character.
  pub async fn corporation_history(&self) -> Result<Vec<CorporationHistoryEntry>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/corporationhistory/", self.id))
          .build(),
        None,
      )
      .await
  }

  /// Returns public information for this character.
  pub async fn detail(&self) -> Result<CharacterDetail, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v5/characters/{}/", self.id))
          .build(),
        None,
      )
      .await
  }

  /// Returns the portrait URLs for this character.
  pub async fn portrait(&self) -> Result<CharacterPortrait, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v3/characters/{}/portrait/", self.id))
          .build(),
        None,
      )
      .await
  }
}

/// Authenticated client for character ESI endpoints requiring a valid OAuth2 grant.
pub struct AuthenticatedClient<'a> {
  pub(in crate::clients::character) esi: &'a EsiClient,
  pub(in crate::clients::character) grant: &'a Grant,
  pub(in crate::clients::character) id: i64,
}

impl<'a> AuthenticatedClient<'a> {
  /// Creates an authenticated client using the character ID from the grant.
  pub(crate) fn new(esi: &'a EsiClient, grant: &'a Grant) -> Self {
    Self {
      esi,
      id: *grant.character_id(),
      grant,
    }
  }
}
