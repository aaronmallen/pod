//! Character mail endpoints.

use crate::{
  Error,
  clients::character::AuthenticatedClient,
  models::character::{MailHeader, MailLabels, MailList, MailMessage, NewMail, NewMailLabel, UpdateMail},
};

impl AuthenticatedClient<'_> {
  /// Deletes a mail message.
  pub async fn delete_mail(&self, mail_id: i64) -> Result<(), Error> {
    self
      .esi
      .http()
      .delete_empty(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/mail/{mail_id}/", self.id))
          .build(),
        self.grant.access_token(),
      )
      .await
  }

  /// Deletes a mail label.
  pub async fn delete_mail_label(&self, label_id: i64) -> Result<(), Error> {
    self
      .esi
      .http()
      .delete_empty(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/mail/labels/{label_id}/", self.id))
          .build(),
        self.grant.access_token(),
      )
      .await
  }

  /// Returns mail headers for this character (paginated).
  pub async fn mail(&self) -> Result<Vec<MailHeader>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/mail/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the mail labels and unread counts for this character.
  pub async fn mail_labels(&self) -> Result<MailLabels, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v3/characters/{}/mail/labels/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the mailing list subscriptions for this character.
  pub async fn mailing_lists(&self) -> Result<Vec<MailList>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/mail/lists/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns the full contents of a mail message.
  pub async fn mail_message(&self, mail_id: i64) -> Result<MailMessage, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/mail/{mail_id}/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Creates a new mail label and returns its ID.
  pub async fn create_mail_label(&self, body: NewMailLabel) -> Result<i64, Error> {
    self
      .esi
      .http()
      .post_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/mail/labels/", self.id))
          .build(),
        &body,
        self.grant.access_token(),
      )
      .await
  }

  /// Sends a new mail and returns its ID.
  pub async fn send_mail(&self, body: NewMail) -> Result<i64, Error> {
    self
      .esi
      .http()
      .post_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/mail/", self.id))
          .build(),
        &body,
        self.grant.access_token(),
      )
      .await
  }

  /// Updates the read state or labels on a mail message.
  pub async fn update_mail(&self, mail_id: i64, body: UpdateMail) -> Result<(), Error> {
    self
      .esi
      .http()
      .put_empty(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/mail/{mail_id}/", self.id))
          .build(),
        &body,
        self.grant.access_token(),
      )
      .await
  }
}
