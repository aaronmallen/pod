//! Character contact endpoints.

use crate::{
  Error,
  clients::character::AuthenticatedClient,
  models::character::{AddContacts, CharacterContact, ContactLabel, UpdateContacts},
};

impl AuthenticatedClient<'_> {
  /// Adds contacts for this character and returns their IDs.
  pub async fn add_contacts(&self, body: AddContacts) -> Result<Vec<i64>, Error> {
    self
      .esi
      .http()
      .post_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/contacts/", self.id))
          .build(),
        &body,
        self.grant.access_token(),
      )
      .await
  }

  /// Returns all contacts for this character (paginated).
  pub async fn contacts(&self) -> Result<Vec<CharacterContact>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/contacts/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns all contact labels for this character.
  pub async fn contact_labels(&self) -> Result<Vec<ContactLabel>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/contacts/labels/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Deletes a list of contacts for this character.
  pub async fn delete_contacts(&self, contact_ids: &[i64]) -> Result<(), Error> {
    let base_url = self
      .esi
      .url_builder()
      .path(format!("v2/characters/{}/contacts/", self.id))
      .build();
    let ids_query: String = contact_ids
      .iter()
      .map(|id| format!("contact_ids%5B%5D={id}"))
      .collect::<Vec<_>>()
      .join("&");
    let url = if ids_query.is_empty() {
      base_url
    } else {
      format!("{base_url}?{ids_query}")
    };
    self.esi.http().delete_empty(&url, self.grant.access_token()).await
  }

  /// Updates contacts for this character.
  pub async fn update_contacts(&self, body: UpdateContacts) -> Result<(), Error> {
    self
      .esi
      .http()
      .put_empty(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/contacts/", self.id))
          .build(),
        &body,
        self.grant.access_token(),
      )
      .await
  }
}
