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

#[cfg(test)]
mod tests {
  use std::time::{Duration, SystemTime};

  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

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

  mod contacts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_contacts_for_character() {
      let server = MockServer::start().await;

      Mock::given(method("GET"))
        .and(path("/v2/characters/90000001/contacts/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([
              {
                "contact_id": 90000002,
                "contact_type": "character",
                "standing": 5.0
              }
            ])),
        )
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.contacts().await.unwrap();

      assert_eq!(result.len(), 1);
      assert_eq!(result[0].contact_id, 90_000_002i64);
      assert_eq!(result[0].contact_type, "character");
      assert_eq!(result[0].standing, 5.0f64);
    }

    #[tokio::test]
    async fn it_returns_error_on_401() {
      let server = MockServer::start().await;

      Mock::given(method("GET"))
        .and(path("/v2/characters/90000001/contacts/"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({"error": "Unauthorized"})))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.contacts().await;

      assert!(matches!(
        result,
        Err(crate::Error::Api {
          status: 401,
          ..
        })
      ));
    }
  }
}
