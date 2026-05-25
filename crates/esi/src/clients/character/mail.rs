//! Character mail endpoints.

use crate::{
  Error,
  clients::character::AuthenticatedClient,
  models::character::{MailHeader, MailLabels, MailList, MailMessage, NewMail, NewMailLabel, UpdateMail},
};

impl AuthenticatedClient<'_> {
  /// Adds a label to a mail message, fetching current labels first to avoid clobbering others.
  pub async fn add_mail_label(&self, mail_id: i64, label_id: i64) -> Result<(), Error> {
    let mail = self.mail_message(mail_id).await?;
    let mut labels: Vec<i64> = mail.labels.unwrap_or_default();
    if !labels.contains(&label_id) {
      labels.push(label_id);
    }
    self
      .update_mail(
        mail_id,
        UpdateMail {
          labels: Some(labels),
          read: None,
        },
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
          .path(format!("v3/characters/{}/mail/labels/", self.id))
          .build(),
        &body,
        self.grant.access_token(),
      )
      .await
  }

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

  /// Returns the ID of the character's "Snoozed" mail label, creating it if it doesn't exist.
  pub async fn ensure_snoozed_label(&self) -> Result<i64, Error> {
    if let Some(id) = self.snoozed_label_id().await? {
      return Ok(id);
    }
    self
      .create_mail_label(NewMailLabel {
        color: Some("#ffaa00".into()),
        name: "Snoozed".into(),
      })
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

  /// Removes a label from a mail message, fetching current labels first to preserve others.
  pub async fn remove_mail_label(&self, mail_id: i64, label_id: i64) -> Result<(), Error> {
    let mail = self.mail_message(mail_id).await?;
    let mut labels: Vec<i64> = mail.labels.unwrap_or_default();
    labels.retain(|&id| id != label_id);
    self
      .update_mail(
        mail_id,
        UpdateMail {
          labels: Some(labels),
          read: None,
        },
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

  /// Returns the ID of the character's "Snoozed" mail label, or `None` if it doesn't exist yet.
  pub async fn snoozed_label_id(&self) -> Result<Option<i64>, Error> {
    let labels = self.mail_labels().await?;
    Ok(
      labels
        .labels
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|l| l.name.as_deref() == Some("Snoozed"))
        .and_then(|l| l.label_id),
    )
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

#[cfg(test)]
mod tests {
  use std::time::{Duration, SystemTime};

  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  fn make_client_and_grant(server: &MockServer) -> (crate::Client, crate::models::auth::Grant) {
    let esi = crate::Client::builder("test-client")
      .base_url(server.uri())
      .build()
      .unwrap();
    let grant = crate::models::auth::Grant::new(
      "test-token",
      90_000_001i64,
      "Test Char",
      SystemTime::now() + Duration::from_secs(3600),
      "refresh",
      vec![],
    );
    (esi, grant)
  }

  fn make_grant() -> crate::models::auth::Grant {
    crate::models::auth::Grant::new(
      "test-token",
      90_000_001i64,
      "Test Char",
      SystemTime::now() + Duration::from_secs(3600),
      "refresh",
      vec![],
    )
  }

  mod add_mail_label {
    use super::*;

    #[tokio::test]
    async fn it_adds_label_to_mail_without_it() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/characters/90000001/mail/100/"))
        .respond_with(
          ResponseTemplate::new(200).set_body_raw(r#"{"labels": [1, 2], "read": true}"#, "application/json"),
        )
        .mount(&server)
        .await;
      Mock::given(method("PUT"))
        .and(path("/v1/characters/90000001/mail/100/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
      let (esi, grant) = make_client_and_grant(&server);
      let auth = esi.character(&grant);

      let result = auth.add_mail_label(100, 42).await;

      assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_does_not_duplicate_label_already_present() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/characters/90000001/mail/100/"))
        .respond_with(
          ResponseTemplate::new(200).set_body_raw(r#"{"labels": [1, 42], "read": true}"#, "application/json"),
        )
        .mount(&server)
        .await;
      Mock::given(method("PUT"))
        .and(path("/v1/characters/90000001/mail/100/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
      let (esi, grant) = make_client_and_grant(&server);
      let auth = esi.character(&grant);

      let result = auth.add_mail_label(100, 42).await;

      assert!(result.is_ok());
    }
  }

  mod ensure_snoozed_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_existing_id_when_label_found() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v3/characters/90000001/mail/labels/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r##"{"labels": [{"label_id": 42, "name": "Snoozed", "color": "#ffaa00", "unread_count": 0}], "total_unread_count": 0}"##,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (esi, grant) = make_client_and_grant(&server);
      let auth = esi.character(&grant);

      let id = auth.ensure_snoozed_label().await.unwrap();

      assert_eq!(id, 42);
    }

    #[tokio::test]
    async fn it_creates_label_when_not_found() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v3/characters/90000001/mail/labels/"))
        .respond_with(
          ResponseTemplate::new(200).set_body_raw(r#"{"labels": [], "total_unread_count": 0}"#, "application/json"),
        )
        .mount(&server)
        .await;
      Mock::given(method("POST"))
        .and(path("/v3/characters/90000001/mail/labels/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("99", "application/json"))
        .mount(&server)
        .await;
      let (esi, grant) = make_client_and_grant(&server);
      let auth = esi.character(&grant);

      let id = auth.ensure_snoozed_label().await.unwrap();

      assert_eq!(id, 99);
    }
  }

  mod mail {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_mail_headers_on_success() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/characters/90000001/mail/"))
        .respond_with(ResponseTemplate::new(200).insert_header("X-Pages", "1").set_body_raw(
          r#"[{"mail_id": 1, "subject": "Test", "from": 90000002, "is_read": false, "recipients": []}]"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.mail().await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].mail_id, Some(1));
      assert_eq!(result[0].subject, Some("Test".to_string()));
      assert_eq!(result[0].from, Some(90000002));
      assert_eq!(result[0].is_read, Some(false));
    }

    #[tokio::test]
    async fn it_returns_api_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/characters/90000001/mail/"))
        .respond_with(ResponseTemplate::new(401).set_body_raw(r#"{"error": "Unauthorized"}"#, "application/json"))
        .mount(&server)
        .await;
      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.mail().await;

      assert!(matches!(
        result,
        Err(crate::Error::Api {
          status: 401,
          ..
        })
      ));
    }
  }

  mod remove_mail_label {
    use super::*;

    #[tokio::test]
    async fn it_removes_label_while_preserving_others() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/characters/90000001/mail/100/"))
        .respond_with(
          ResponseTemplate::new(200).set_body_raw(r#"{"labels": [1, 42, 7], "read": true}"#, "application/json"),
        )
        .mount(&server)
        .await;
      Mock::given(method("PUT"))
        .and(path("/v1/characters/90000001/mail/100/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
      let (esi, grant) = make_client_and_grant(&server);
      let auth = esi.character(&grant);

      let result = auth.remove_mail_label(100, 42).await;

      assert!(result.is_ok());
    }

    #[tokio::test]
    async fn it_succeeds_when_label_is_absent() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/characters/90000001/mail/100/"))
        .respond_with(
          ResponseTemplate::new(200).set_body_raw(r#"{"labels": [1, 7], "read": true}"#, "application/json"),
        )
        .mount(&server)
        .await;
      Mock::given(method("PUT"))
        .and(path("/v1/characters/90000001/mail/100/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
      let (esi, grant) = make_client_and_grant(&server);
      let auth = esi.character(&grant);

      let result = auth.remove_mail_label(100, 42).await;

      assert!(result.is_ok());
    }
  }

  mod snoozed_label_id {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_id_when_snoozed_label_exists() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v3/characters/90000001/mail/labels/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r##"{"labels": [{"label_id": 42, "name": "Snoozed", "color": "#ffaa00", "unread_count": 0}], "total_unread_count": 0}"##,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (esi, grant) = make_client_and_grant(&server);
      let auth = esi.character(&grant);

      let result = auth.snoozed_label_id().await.unwrap();

      assert_eq!(result, Some(42));
    }

    #[tokio::test]
    async fn it_returns_none_when_snoozed_label_absent() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v3/characters/90000001/mail/labels/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r##"{"labels": [{"label_id": 1, "name": "Inbox", "color": "#ffffff", "unread_count": 0}], "total_unread_count": 0}"##,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (esi, grant) = make_client_and_grant(&server);
      let auth = esi.character(&grant);

      let result = auth.snoozed_label_id().await.unwrap();

      assert_eq!(result, None);
    }
  }
}
