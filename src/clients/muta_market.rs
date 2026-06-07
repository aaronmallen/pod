use std::{
  sync::Arc,
  time::{Duration, Instant},
};

use serde::Deserialize;
use tokio::sync::Mutex;

use crate::clients::{self, user_agent};

const BASE_URL: &str = "https://mutamarket.com/api/modules";

const MAX_RETRIES: u32 = 2;

const RETRY_DELAY: Duration = Duration::from_secs(2);

const THROTTLE: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct Client {
  base_url: String,
  http: reqwest::Client,
  last_request: Arc<Mutex<Option<Instant>>>,
}

impl Client {
  pub fn new() -> Self {
    Self::with_base_url_inner(BASE_URL)
  }

  #[cfg(test)]
  pub fn with_base_url(base_url: impl Into<String>) -> Self {
    Self::with_base_url_inner(base_url)
  }

  fn with_base_url_inner(base_url: impl Into<String>) -> Self {
    Self {
      base_url: base_url.into(),
      http: reqwest::Client::builder()
        .user_agent(user_agent())
        .build()
        .unwrap_or_default(),
      last_request: Arc::new(Mutex::new(None)),
    }
  }

  pub async fn item_price(&self, item_id: i64) -> Result<Option<f64>, clients::Error> {
    let mut last_err = None;
    for attempt in 0..=MAX_RETRIES {
      if attempt > 0 {
        tokio::time::sleep(RETRY_DELAY).await;
      }
      match self.fetch_once(item_id).await {
        Ok(price) => return Ok(price),
        Err(error) if is_transient(&error) => {
          tracing::warn!(
            item_id,
            attempt = attempt + 1,
            max_attempts = MAX_RETRIES + 1,
            "mutamarket: transient error fetching price: {error}"
          );
          last_err = Some(error);
        }
        Err(error) => return Err(clients::Error::Http(error)),
      }
    }
    Err(clients::Error::Http(last_err.expect("retry loop ran at least once")))
  }

  async fn fetch_once(&self, item_id: i64) -> Result<Option<f64>, reqwest::Error> {
    self.throttle().await;
    let url = format!("{}/{item_id}", self.base_url.trim_end_matches('/'));
    let response = self.http.get(&url).send().await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
      return Ok(None);
    }
    let appraisal: ModuleAppraisal = response.error_for_status()?.json().await?;
    Ok(appraisal.estimated_value)
  }

  async fn throttle(&self) {
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

impl Default for Client {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(Deserialize)]
struct ModuleAppraisal {
  estimated_value: Option<f64>,
}

fn is_transient(error: &reqwest::Error) -> bool {
  if error.is_timeout() || error.is_connect() {
    return true;
  }
  error.status().is_some_and(|status| status.is_server_error())
}

#[cfg(test)]
mod tests {
  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
  };

  use super::*;

  mod item_price {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_the_estimated_value() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/100"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"estimated_value":1234.5}"#, "application/json"))
        .mount(&server)
        .await;
      let client = Client::with_base_url(server.uri());

      let price = client.item_price(100).await.unwrap();

      assert_eq!(price, Some(1234.5));
    }

    #[tokio::test]
    async fn it_returns_none_for_an_unlisted_item() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/404"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
      let client = Client::with_base_url(server.uri());

      let price = client.item_price(404).await.unwrap();

      assert_eq!(price, None);
    }

    #[tokio::test]
    async fn it_returns_none_when_the_item_has_no_estimate() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/7"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"estimated_value":null}"#, "application/json"))
        .mount(&server)
        .await;
      let client = Client::with_base_url(server.uri());

      let price = client.item_price(7).await.unwrap();

      assert_eq!(price, None);
    }

    #[tokio::test]
    async fn it_errors_on_a_persistent_server_error() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/9"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
      let client = Client::with_base_url(server.uri());

      let result = client.item_price(9).await;

      assert!(matches!(result, Err(clients::Error::Http(_))));
    }
  }

  mod throttle {
    use super::*;

    #[tokio::test]
    async fn it_spaces_consecutive_requests() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"estimated_value":1.0}"#, "application/json"))
        .mount(&server)
        .await;
      let client = Client::with_base_url(server.uri());

      let start = Instant::now();
      client.item_price(1).await.unwrap();
      client.item_price(2).await.unwrap();

      assert!(
        start.elapsed() >= THROTTLE,
        "the second request waits out the throttle window"
      );
    }
  }
}
