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

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  fn make_esi(server_uri: &str) -> crate::Client {
    crate::Client::builder("test-client")
      .base_url(server_uri)
      .build()
      .unwrap()
  }

  mod structure {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_a_structure_client() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v2/universe/structures/1035466617946/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "name": "Jita 4-4 CNAP",
          "owner_id": 1000134i64,
          "position": null,
          "solar_system_id": 30000142i64,
          "type_id": null
        })))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let grant = crate::models::auth::Grant::new(
        "test-token",
        123_456_789i64,
        "Test Pilot",
        std::time::SystemTime::now() + std::time::Duration::from_secs(3600),
        "refresh",
        vec![],
      );
      let universe = esi.universe();
      let structure = universe.structure(1_035_466_617_946i64);
      let detail = structure.auth(&grant).detail().await.unwrap();

      assert_eq!(detail.name, "Jita 4-4 CNAP");
      assert_eq!(detail.owner_id, 1_000_134i64);
    }

    #[tokio::test]
    async fn it_returns_error_on_404() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v2/universe/structures/1035466617946/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"error": "Not found"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let grant = crate::models::auth::Grant::new(
        "test-token",
        123_456_789i64,
        "Test Pilot",
        std::time::SystemTime::now() + std::time::Duration::from_secs(3600),
        "refresh",
        vec![],
      );
      let universe = esi.universe();
      let structure = universe.structure(1_035_466_617_946i64);
      let result = structure.auth(&grant).detail().await;

      assert!(result.is_err());
    }
  }
}
