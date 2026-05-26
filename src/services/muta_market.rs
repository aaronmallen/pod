//! MutaMarket HTTP client for abyssal module price lookups.

use std::{
  sync::Arc,
  time::{Duration, Instant},
};

use serde::Deserialize;
use tokio::sync::Mutex;

const BASE_URL: &str = "https://mutamarket.com/api/modules";

/// A single price estimate entry from the MutaMarket API.
#[derive(Deserialize)]
struct MutaMarketItem {
  estimated_value: Option<f64>,
}

/// HTTP client for MutaMarket. Self-throttles to 1 request per second.
#[derive(Clone)]
pub struct Client {
  http: reqwest::Client,
  last_request: Arc<Mutex<Option<Instant>>>,
}

impl Client {
  /// Creates a new `Client`.
  pub fn new() -> Self {
    Self {
      http: reqwest::Client::new(),
      last_request: Arc::new(Mutex::new(None)),
    }
  }

  /// Returns the estimated ISK price for the given abyssal item, or `None`
  /// if the item is not listed on MutaMarket. Throttles to 1 req/s.
  pub async fn item_price(&self, item_id: i64) -> Result<Option<f64>, reqwest::Error> {
    self.throttle().await;
    let url = format!("{BASE_URL}/{item_id}");
    let response = self.http.get(&url).send().await?;
    parse_item_price_response(response).await
  }

  /// Enforces a 1-second gap between requests.
  async fn throttle(&self) {
    let mut guard = self.last_request.lock().await;
    if let Some(last) = *guard {
      let elapsed = last.elapsed();
      if elapsed < Duration::from_secs(1) {
        tokio::time::sleep(Duration::from_secs(1) - elapsed).await;
      }
    }
    *guard = Some(Instant::now());
  }
}

async fn parse_item_price_response(response: reqwest::Response) -> Result<Option<f64>, reqwest::Error> {
  if response.status() == reqwest::StatusCode::NOT_FOUND {
    return Ok(None);
  }
  let item: MutaMarketItem = response.error_for_status()?.json().await?;
  Ok(item.estimated_value)
}

impl Default for Client {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod new {
    use super::*;

    #[test]
    fn it_creates_a_client() {
      let client = Client::new();
      assert!(client.last_request.try_lock().unwrap().is_none());
    }
  }
}
