use std::{
  collections::HashMap,
  sync::Arc,
  time::{Duration, Instant},
};

use serde::Deserialize;
use tokio::sync::Mutex;

use crate::clients::{self, http};

const BASE_URL: &str = "https://zkillboard.com/api";

const MAX_RETRIES: u32 = 2;

const RETRY_DELAY: Duration = Duration::from_secs(2);

const THROTTLE: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct Client {
  base_url: String,
  http: Arc<http::Client>,
  last_request: Arc<Mutex<Option<Instant>>>,
}

impl Client {
  pub fn new(http: Arc<http::Client>) -> Self {
    Self::with_base_url_inner(http, BASE_URL)
  }

  #[cfg(test)]
  pub fn with_base_url(http: Arc<http::Client>, base_url: impl Into<String>) -> Self {
    Self::with_base_url_inner(http, base_url)
  }

  fn with_base_url_inner(http: Arc<http::Client>, base_url: impl Into<String>) -> Self {
    Self {
      base_url: base_url.into(),
      http,
      last_request: Arc::new(Mutex::new(None)),
    }
  }

  pub async fn character_kills(&self, character_id: i64) -> Result<Vec<Killmail>, clients::Error> {
    self.feed(&format!("characterID/{character_id}/kills/")).await
  }

  pub async fn character_losses(&self, character_id: i64) -> Result<Vec<Killmail>, clients::Error> {
    self.feed(&format!("characterID/{character_id}/losses/")).await
  }

  pub async fn prices(&self, type_id: i64) -> Result<Option<f64>, clients::Error> {
    let path = format!("prices/{type_id}/");
    let mut last_err = None;
    for attempt in 0..=MAX_RETRIES {
      if attempt > 0 {
        tokio::time::sleep(RETRY_DELAY).await;
      }
      match self.fetch_prices_once(&path).await {
        Ok(history) => return Ok(latest_price(&history)),
        Err(error) if is_transient(&error) => {
          tracing::warn!(
            path,
            attempt = attempt + 1,
            max_attempts = MAX_RETRIES + 1,
            "zkillboard: transient error fetching prices: {error}"
          );
          last_err = Some(error);
        }
        Err(error) => return Err(error),
      }
    }
    Err(last_err.expect("retry loop ran at least once"))
  }

  pub async fn value_for_kill(&self, killmail_id: i64) -> Result<Option<f64>, clients::Error> {
    let killmails = self.feed(&format!("killID/{killmail_id}/")).await?;
    Ok(killmails.first().map(|killmail| killmail.zkb.total_value))
  }

  async fn feed(&self, path: &str) -> Result<Vec<Killmail>, clients::Error> {
    let mut last_err = None;
    for attempt in 0..=MAX_RETRIES {
      if attempt > 0 {
        tokio::time::sleep(RETRY_DELAY).await;
      }
      match self.fetch_once(path).await {
        Ok(killmails) => return Ok(killmails),
        Err(error) if is_transient(&error) => {
          tracing::warn!(
            path,
            attempt = attempt + 1,
            max_attempts = MAX_RETRIES + 1,
            "zkillboard: transient error fetching feed: {error}"
          );
          last_err = Some(error);
        }
        Err(error) => return Err(error),
      }
    }
    Err(last_err.expect("retry loop ran at least once"))
  }

  async fn fetch_once(&self, path: &str) -> Result<Vec<Killmail>, clients::Error> {
    self.throttle().await;
    let url = format!("{}/{path}", self.base_url.trim_end_matches('/'));
    self.http.get_json::<Vec<Killmail>>(&url, None, None).await
  }

  async fn fetch_prices_once(&self, path: &str) -> Result<PriceHistory, clients::Error> {
    self.throttle().await;
    let url = format!("{}/{path}", self.base_url.trim_end_matches('/'));
    self.http.get_json::<PriceHistory>(&url, None, None).await
  }

  async fn throttle(&self) {
    // Even though requests go through the shared rate-limited http::Client, zKillboard's THROTTLE
    // must remain local: the shared budgets are driven by ESI's X-Ratelimit-* headers, which
    // zKillboard does not send, so the ~1 req/s etiquette must be enforced here.
    let mut guard = self.last_request.lock().await;
    if let Some(last) = *guard {
      let elapsed = last.elapsed();
      if elapsed < THROTTLE {
        tokio::time::sleep(THROTTLE - elapsed).await;
      }
    }
    *guard = Some(Instant::now());
  }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Killmail {
  pub killmail_id: i64,
  pub zkb: Zkb,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Zkb {
  pub hash: String,
  #[serde(rename = "totalValue", default)]
  pub total_value: f64,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PriceHistory {
  Dated(HashMap<String, serde_json::Value>),
  Other(serde::de::IgnoredAny),
}

fn latest_price(history: &PriceHistory) -> Option<f64> {
  let PriceHistory::Dated(map) = history else {
    return None;
  };
  map
    .iter()
    .filter(|(key, _)| !key.eq_ignore_ascii_case("typeid"))
    .filter(|(key, _)| key.starts_with(|c: char| c.is_ascii_digit()))
    .max_by(|(left, _), (right, _)| left.cmp(right))
    .and_then(|(_, value)| value.as_f64())
    .filter(|price| *price > 0.0)
}

fn is_transient(error: &clients::Error) -> bool {
  match error {
    clients::Error::Http(e) => {
      e.is_timeout() || e.is_connect() || e.status().is_some_and(|status| status.is_server_error())
    }
    _ => false,
  }
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;
  use crate::store;

  async fn make_http() -> Arc<http::Client> {
    let db = store::open_test().await.unwrap();
    http::Client::builder(http::Cache::new(db)).build()
  }

  mod character_kills {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_defaults_total_value_when_absent() {
      let server = MockServer::start().await;
      let body = r#"[{"killmail_id": 200, "zkb": {"hash": "ghi789"}}]"#;
      Mock::given(method("GET"))
        .and(path("/characterID/42/kills/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let client = Client::with_base_url(make_http().await, server.uri());

      let kills = client.character_kills(42).await.unwrap();

      assert_eq!(kills[0].zkb.total_value, 0.0);
    }

    #[tokio::test]
    async fn it_errors_on_a_persistent_server_error() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/characterID/42/kills/"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let client = Client::with_base_url(make_http().await, server.uri());

      let result = client.character_kills(42).await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }

    #[tokio::test]
    async fn it_returns_killmail_rows_with_hash_and_value() {
      let server = MockServer::start().await;
      let body = r#"[
        {"killmail_id": 100, "zkb": {"hash": "abc123", "totalValue": 1234.5}},
        {"killmail_id": 101, "zkb": {"hash": "def456", "totalValue": 9999.0}}
      ]"#;
      Mock::given(method("GET"))
        .and(path("/characterID/42/kills/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let client = Client::with_base_url(make_http().await, server.uri());

      let kills = client.character_kills(42).await.unwrap();

      assert_eq!(kills.len(), 2);
      assert_eq!(kills[0].killmail_id, 100);
      assert_eq!(kills[0].zkb.hash, "abc123");
      assert_eq!(kills[0].zkb.total_value, 1234.5);
    }
  }

  mod character_losses {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_fetches_the_losses_feed() {
      let server = MockServer::start().await;
      let body = r#"[{"killmail_id": 300, "zkb": {"hash": "loss1", "totalValue": 50.0}}]"#;
      Mock::given(method("GET"))
        .and(path("/characterID/42/losses/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let client = Client::with_base_url(make_http().await, server.uri());

      let losses = client.character_losses(42).await.unwrap();

      assert_eq!(losses.len(), 1);
      assert_eq!(losses[0].killmail_id, 300);
    }
  }

  mod prices {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn mount(server: &MockServer, type_id: i64, body: &str) {
      Mock::given(method("GET"))
        .and(path(format!("/prices/{type_id}/")))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body.to_string(), "application/json"))
        .mount(server)
        .await;
    }

    #[tokio::test]
    async fn it_ignores_the_type_id_key_and_returns_the_latest_dated_price() {
      let server = MockServer::start().await;
      let body = r#"{"typeID": 670, "2024-01-01": 100.0, "2024-03-15": 250.5, "2024-02-01": 200.0}"#;
      mount(&server, 670, body).await;
      let client = Client::with_base_url(make_http().await, server.uri());

      let value = client.prices(670).await.unwrap();

      assert_eq!(value, Some(250.5));
    }

    #[tokio::test]
    async fn it_returns_none_for_an_empty_array_response() {
      let server = MockServer::start().await;
      mount(&server, 999, "[]").await;
      let client = Client::with_base_url(make_http().await, server.uri());

      let value = client.prices(999).await.unwrap();

      assert_eq!(value, None);
    }

    #[tokio::test]
    async fn it_returns_none_when_only_the_type_id_key_is_present() {
      let server = MockServer::start().await;
      mount(&server, 670, r#"{"typeID": 670}"#).await;
      let client = Client::with_base_url(make_http().await, server.uri());

      let value = client.prices(670).await.unwrap();

      assert_eq!(value, None);
    }

    #[tokio::test]
    async fn it_treats_a_zero_price_as_absent() {
      let server = MockServer::start().await;
      mount(&server, 670, r#"{"typeID": 670, "2024-01-01": 0.0}"#).await;
      let client = Client::with_base_url(make_http().await, server.uri());

      let value = client.prices(670).await.unwrap();

      assert_eq!(value, None);
    }
  }

  mod throttle {
    use super::*;

    #[tokio::test]
    async fn it_spaces_consecutive_requests() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;
      let client = Client::with_base_url(make_http().await, server.uri());

      let start = Instant::now();
      client.character_kills(1).await.unwrap();
      client.character_kills(2).await.unwrap();

      assert!(
        start.elapsed() >= THROTTLE,
        "the second request waits out the throttle window"
      );
    }
  }

  mod value_for_kill {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_when_the_kill_is_absent() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/killID/500/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .mount(&server)
        .await;
      let client = Client::with_base_url(make_http().await, server.uri());

      let value = client.value_for_kill(500).await.unwrap();

      assert_eq!(value, None);
    }

    #[tokio::test]
    async fn it_returns_the_total_value_when_the_kill_is_present() {
      let server = MockServer::start().await;
      let body = r#"[{"killmail_id": 500, "zkb": {"hash": "abc123", "totalValue": 4242.5}}]"#;
      Mock::given(method("GET"))
        .and(path("/killID/500/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;
      let client = Client::with_base_url(make_http().await, server.uri());

      let value = client.value_for_kill(500).await.unwrap();

      assert_eq!(value, Some(4242.5));
    }
  }
}
