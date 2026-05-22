//! Character search endpoint.

use serde::Deserialize;

use crate::{Error, clients::character::AuthenticatedClient};

#[derive(Debug, Default, Deserialize)]
struct CharacterSearchResult {
  character: Option<Vec<i64>>,
}

impl AuthenticatedClient<'_> {
  /// Searches for characters by name prefix (minimum 3 characters).
  /// Returns up to the first 20 matching character IDs.
  pub async fn search_characters(&self, query: &str) -> Result<Vec<i64>, Error> {
    let url = self
      .esi
      .url_builder()
      .path(format!("v3/characters/{}/search/", self.id))
      .param("categories", "character")
      .param("search", query)
      .build();
    let result: CharacterSearchResult = self.esi.http().get_json(&url, Some(self.grant.access_token())).await?;
    Ok(result.character.unwrap_or_default())
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

  mod search_characters {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_character_ids_on_success() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v3/characters/90000001/search/"))
        .respond_with(
          ResponseTemplate::new(200).set_body_raw(r#"{"character": [90000002, 90000003]}"#, "application/json"),
        )
        .mount(&server)
        .await;
      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.search_characters("Test").await.unwrap();

      assert_eq!(result, vec![90000002, 90000003]);
    }

    #[tokio::test]
    async fn it_returns_empty_vec_on_empty_result() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v3/characters/90000001/search/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{}"#, "application/json"))
        .mount(&server)
        .await;
      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.search_characters("Test").await.unwrap();

      assert_eq!(result, Vec::<i64>::new());
    }

    #[tokio::test]
    async fn it_returns_api_error_on_401() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v3/characters/90000001/search/"))
        .respond_with(ResponseTemplate::new(401).set_body_raw(r#"{"error": "Unauthorized"}"#, "application/json"))
        .mount(&server)
        .await;
      let esi = crate::Client::builder("test-client")
        .base_url(server.uri())
        .build()
        .unwrap();
      let grant = make_grant();
      let auth = esi.character(&grant);

      let result = auth.search_characters("Test").await;

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
