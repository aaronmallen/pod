//! Character clones, implants, and fittings endpoints.

use crate::{
  Error,
  clients::character::AuthenticatedClient,
  models::character::{Clones, Fitting, FittingId, NewFitting},
};

impl AuthenticatedClient<'_> {
  /// Returns clone information for this character.
  pub async fn clones(&self) -> Result<Clones, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v4/characters/{}/clones/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Creates a new fitting and returns its ID.
  pub async fn create_fitting(&self, body: NewFitting) -> Result<FittingId, Error> {
    self
      .esi
      .http()
      .post_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/fittings/", self.id))
          .build(),
        &body,
        self.grant.access_token(),
      )
      .await
  }

  /// Deletes a saved fitting.
  pub async fn delete_fitting(&self, fitting_id: i64) -> Result<(), Error> {
    self
      .esi
      .http()
      .delete_empty(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/fittings/{fitting_id}/", self.id))
          .build(),
        self.grant.access_token(),
      )
      .await
  }

  /// Returns saved fittings for this character.
  pub async fn fittings(&self) -> Result<Vec<Fitting>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v2/characters/{}/fittings/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }

  /// Returns active implants for this character.
  pub async fn implants(&self) -> Result<Vec<i32>, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/characters/{}/implants/", self.id))
          .build(),
        Some(self.grant.access_token()),
      )
      .await
  }
}

#[cfg(test)]
mod tests {
  fn make_esi(server_uri: &str) -> (crate::Client, crate::models::auth::Grant) {
    let esi = crate::Client::builder("test-client")
      .base_url(server_uri)
      .build()
      .unwrap();
    let grant = crate::models::auth::Grant::new(
      "test-token",
      90_000_001i64,
      "Test Char",
      std::time::SystemTime::now() + std::time::Duration::from_secs(3600),
      "refresh",
      vec![],
    );
    (esi, grant)
  }

  mod clones {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;

    #[tokio::test]
    async fn it_returns_clones_on_success() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v4/characters/90000001/clones/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
          r#"{"jump_clones":[],"last_clone_jump_date":null,"last_station_change_date":null}"#,
          "application/json",
        ))
        .mount(&server)
        .await;
      let (esi, grant) = make_esi(&server.uri());
      let auth = esi.character(&grant);

      let result = auth.clones().await.unwrap();

      assert_eq!(result.jump_clones.len(), 0);
      assert!(result.last_clone_jump_date.is_none());
      assert!(result.last_station_change_date.is_none());
    }

    #[tokio::test]
    async fn it_returns_api_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v4/characters/90000001/clones/"))
        .respond_with(ResponseTemplate::new(401).set_body_raw(r#"{"error":"Unauthorized"}"#, "application/json"))
        .mount(&server)
        .await;
      let (esi, grant) = make_esi(&server.uri());
      let auth = esi.character(&grant);

      let result = auth.clones().await;

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
