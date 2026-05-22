//! Client for EVE killmail ESI endpoints.

use crate::{Error, models::killmail::Killmail};

/// Client for a specific killmail.
pub struct Client<'a> {
  esi: &'a crate::Client,
  hash: String,
  id: i64,
}

impl<'a> Client<'a> {
  /// Creates a new `Client` for the killmail with the given `id` and `hash`.
  pub(crate) fn new(esi: &'a crate::Client, id: i64, hash: &str) -> Self {
    Self {
      esi,
      hash: hash.to_owned(),
      id,
    }
  }

  /// Returns the killmail details.
  pub async fn detail(&self) -> Result<Killmail, Error> {
    self
      .esi
      .http()
      .get_json(
        &self
          .esi
          .url_builder()
          .path(format!("v1/killmails/{}/{}/", self.id, self.hash))
          .build(),
        None,
      )
      .await
  }
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

  mod detail {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_killmail_detail() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/killmails/12345/abc123def/"))
        .respond_with(
          ResponseTemplate::new(200)
            .insert_header("X-Pages", "1")
            .set_body_json(serde_json::json!({
              "attackers": [],
              "killmail_id": 12345,
              "killmail_time": "2024-01-15T10:00:00Z",
              "moon_id": null,
              "solar_system_id": 30000142,
              "victim": {},
              "war_id": null
            })),
        )
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let km = esi.killmail(12345, "abc123def").detail().await.unwrap();

      assert_eq!(km.killmail_id, 12345);
      assert_eq!(km.solar_system_id, 30000142);
      assert_eq!(km.killmail_time, "2024-01-15T10:00:00Z");
      assert!(km.moon_id.is_none());
      assert!(km.war_id.is_none());
    }

    #[tokio::test]
    async fn it_returns_error_on_api_failure() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/v1/killmails/99999/badhash/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({"error": "Killmail not found"})))
        .mount(&server)
        .await;

      let esi = make_esi(&server.uri());
      let result = esi.killmail(99999, "badhash").detail().await;

      assert!(result.is_err());
    }
  }
}
