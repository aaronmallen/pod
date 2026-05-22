//! Corporation contact endpoints.

use crate::{
  Error,
  clients::corporation::AuthenticatedClient,
  models::corporation::{CorporationContact, CorporationContactLabel},
};

impl AuthenticatedClient<'_> {
  /// Returns all contacts for this corporation (paginated).
  pub async fn contacts(&self) -> Result<Vec<CorporationContact>, Error> {
    self
      .esi
      .http()
      .get_json_paginated(
        &self
          .esi
          .url_builder()
          .path(format!("v2/corporations/{}/contacts/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns all contact labels for this corporation.
  pub async fn contact_labels(&self) -> Result<Vec<CorporationContactLabel>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/corporations/{}/contacts/labels/", self.id))
          .build(),
        Some(self.grant.access_token()),
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
      123_456_789i64,
      "Test Member",
      SystemTime::now() + Duration::from_secs(3600),
      "refresh",
      vec![],
    )
  }

  mod contacts {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_corporation_contacts() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v2/corporations/109299958/contacts/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!([
              {
                "contact_id": 2_112_625_428i64,
                "contact_type": "character",
                "is_watched": null,
                "label_ids": null,
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
      let corp = esi.corporation(109_299_958i64);
      let auth = corp.auth(&grant);

      let contacts = auth.contacts().await.unwrap();

      assert_eq!(contacts.len(), 1);
      assert_eq!(contacts[0].contact_id, 2_112_625_428i64);
      assert_eq!(contacts[0].contact_type, "character");
      assert_eq!(contacts[0].standing, 5.0);
    }

    #[tokio::test]
    async fn it_returns_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v2/corporations/109299958/contacts/"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({"error": "Unauthorized"})))
        .mount(&server)
        .await;

      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let corp = esi.corporation(109_299_958i64);
      let auth = corp.auth(&grant);

      let result = auth.contacts().await;

      assert!(result.is_err());
    }
  }
}
