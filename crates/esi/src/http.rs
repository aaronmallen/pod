//! HTTP-layer utilities for ESI request construction and rate limiting.

use std::{
  collections::BTreeMap,
  sync::{Arc, Mutex},
  time::{Duration, Instant},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use url::Url;

use crate::{Error, cache::Store as CacheStore};

/// ESI response header carrying the number of error-budget requests remaining.
const ESI_ERROR_REMAIN_HEADER: &str = "X-Esi-Error-Limit-Remain";
/// ESI response header carrying the seconds until the error-budget window resets.
const ESI_ERROR_RESET_HEADER: &str = "X-Esi-Error-Limit-Reset";
/// ESI response header carrying the total number of result pages for a paginated endpoint.
const ESI_PAGES_HEADER: &str = "X-Pages";
/// Standard HTTP response header indicating how many seconds the client should wait before retrying.
const RETRY_AFTER_HEADER: &str = "Retry-After";

/// Shared HTTP client used by all ESI callers, bundling a cache, a reqwest client, and a rate limiter.
#[derive(Clone, Debug)]
pub(crate) struct Client {
  cache: CacheStore,
  inner: reqwest::Client,
  rate_limit: Arc<RateLimiter>,
}

impl Client {
  /// Creates a new `Client` with the given cache store, reqwest client, and rate limiter.
  pub fn new(cache: CacheStore, inner: reqwest::Client, rate_limit: Arc<RateLimiter>) -> Self {
    Self {
      cache,
      inner,
      rate_limit,
    }
  }

  /// Fetches a URL and returns the raw response bytes.
  #[tracing::instrument(skip(self))]
  pub async fn get_bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
    self.rate_limit.check().await;
    tracing::trace!(method = "GET", url = url, "esi: request");
    let start = Instant::now();
    let resp = self.inner.get(url).send().await?;
    self.rate_limit.update_from_response(&resp);
    let status = resp.status().as_u16();
    tracing::trace!(
      method = "GET",
      url = url,
      status = status,
      elapsed_ms = start.elapsed().as_millis() as u64,
      "esi: response"
    );
    if (200u16..300).contains(&status) {
      return Ok(resp.bytes().await?.to_vec());
    }
    Err(self.map_error_status(status, resp).await)
  }

  /// Streams the response at `url` directly to a file at `dest`.
  ///
  /// `timeout_secs` sets a per-request deadline — use a long value (e.g. 600)
  /// for large downloads where the default request timeout would be too short.
  #[tracing::instrument(skip(self, dest))]
  pub async fn download_to_file(&self, url: &str, dest: &std::path::Path, timeout_secs: u64) -> Result<(), Error> {
    self.rate_limit.check().await;
    tracing::trace!(method = "GET", url = url, "esi: request");
    let start = Instant::now();
    let mut resp = self
      .inner
      .get(url)
      .timeout(Duration::from_secs(timeout_secs))
      .send()
      .await?;
    self.rate_limit.update_from_response(&resp);

    let status = resp.status().as_u16();
    tracing::trace!(
      method = "GET",
      url = url,
      status = status,
      elapsed_ms = start.elapsed().as_millis() as u64,
      "esi: response"
    );
    if status == 420 {
      let secs = retry_after_secs(&resp);
      tracing::warn!(url = url, status = 420, retry_after_secs = secs, "esi: rate limited");
      return Err(self.rate_limit.handle_420(secs).await);
    }
    if !(200u16..300).contains(&status) {
      return Err(api_error(resp).await);
    }

    stream_response_to_file(&mut resp, dest).await
  }

  /// Sends an authenticated DELETE request and discards the response body.
  #[tracing::instrument(skip(self, token))]
  pub async fn delete_empty(&self, url: &str, token: &str) -> Result<(), Error> {
    self.rate_limit.check().await;
    tracing::trace!(method = "DELETE", url = url, "esi: request");
    let start = Instant::now();
    let resp = self.inner.delete(url).bearer_auth(token).send().await?;
    self.rate_limit.update_from_response(&resp);
    let status = resp.status().as_u16();
    tracing::trace!(
      method = "DELETE",
      url = url,
      status = status,
      elapsed_ms = start.elapsed().as_millis() as u64,
      "esi: response"
    );
    if (200u16..300).contains(&status) {
      return Ok(());
    }
    Err(self.map_error_status(status, resp).await)
  }

  /// Fetches a single JSON resource, using a cached ETag to avoid unnecessary data transfer on 304 responses.
  #[tracing::instrument(skip(self, token))]
  pub async fn get_json<T: DeserializeOwned>(&self, url: &str, token: Option<&str>) -> Result<T, Error> {
    self.rate_limit.check().await;

    let cached = self.cache.get(url);
    let mut req = self.inner.get(url);
    if let Some(ref entry) = cached {
      req = req.header("If-None-Match", &entry.0);
    }
    if let Some(t) = token {
      req = req.bearer_auth(t);
    }

    tracing::trace!(method = "GET", url = url, cached = cached.is_some(), "esi: request");
    let start = Instant::now();
    let resp = req.send().await?;
    self.rate_limit.update_from_response(&resp);
    let status = resp.status().as_u16();
    tracing::trace!(
      method = "GET",
      url = url,
      status = status,
      elapsed_ms = start.elapsed().as_millis() as u64,
      "esi: response"
    );

    if status == 304 {
      let body = cached.expect("304 requires a cached entry").1;
      return Ok(serde_json::from_slice(&body)?);
    }

    if (200u16..300).contains(&status) {
      return self.consume_json_with_etag(url, resp).await;
    }

    Err(self.map_error_status(status, resp).await)
  }

  /// Fetches all pages of a paginated ESI endpoint concurrently and merges them into a single `Vec`.
  ///
  /// Page 1 is fetched first to read the `X-Pages` header; remaining pages are dispatched in
  /// parallel via a `JoinSet`.
  #[tracing::instrument(skip(self, token))]
  pub async fn get_json_paginated<T: DeserializeOwned + Send + 'static>(
    &self,
    url_base: &str,
    token: Option<&str>,
  ) -> Result<Vec<T>, Error> {
    let page1_url = UrlBuilder::new(url_base).param("page", "1").build();
    self.rate_limit.check().await;

    let mut req = self.inner.get(&page1_url);
    if let Some(t) = token {
      req = req.bearer_auth(t);
    }

    tracing::trace!(method = "GET", url = url_base, page = 1, "esi: request");
    let start = Instant::now();
    let resp = req.send().await?;
    self.rate_limit.update_from_response(&resp);
    let status = resp.status().as_u16();
    tracing::trace!(
      method = "GET",
      url = url_base,
      status = status,
      elapsed_ms = start.elapsed().as_millis() as u64,
      "esi: response"
    );

    if status == 420 {
      let secs = retry_after_secs(&resp);
      tracing::warn!(
        url = url_base,
        status = 420,
        retry_after_secs = secs,
        "esi: rate limited"
      );
      return Err(self.rate_limit.handle_420(secs).await);
    }
    if !(200u16..300).contains(&status) {
      return Err(api_error(resp).await);
    }

    let total_pages = parse_x_pages_header(&resp);
    let body = resp.bytes().await?;
    let mut results: Vec<T> = serde_json::from_slice(&body)?;

    if total_pages > 1 {
      let remaining = self.fetch_remaining_pages(url_base, token, total_pages).await?;
      results.extend(remaining);
    }

    Ok(results)
  }

  /// Sends an authenticated JSON POST and discards the response body.
  #[tracing::instrument(skip(self, body, token))]
  pub async fn post_empty<B: Serialize>(&self, url: &str, body: &B, token: &str) -> Result<(), Error> {
    self.rate_limit.check().await;
    tracing::trace!(method = "POST", url = url, "esi: request");
    let start = Instant::now();
    let resp = self.inner.post(url).bearer_auth(token).json(body).send().await?;
    self.rate_limit.update_from_response(&resp);
    let status = resp.status().as_u16();
    tracing::trace!(
      method = "POST",
      url = url,
      status = status,
      elapsed_ms = start.elapsed().as_millis() as u64,
      "esi: response"
    );
    if (200u16..300).contains(&status) {
      return Ok(());
    }
    Err(self.map_error_status(status, resp).await)
  }

  /// Sends an unauthenticated form-encoded POST and deserializes the JSON response.
  #[tracing::instrument(skip(self, body))]
  pub async fn post_form_anon<B: Serialize, T: DeserializeOwned>(&self, url: &str, body: &B) -> Result<T, Error> {
    self.rate_limit.check().await;
    tracing::trace!(method = "POST", url = url, "esi: request");
    let start = Instant::now();
    let resp = self.inner.post(url).form(body).send().await?;
    self.rate_limit.update_from_response(&resp);
    let status = resp.status().as_u16();
    tracing::trace!(
      method = "POST",
      url = url,
      status = status,
      elapsed_ms = start.elapsed().as_millis() as u64,
      "esi: response"
    );
    consume_json_response(resp, &self.rate_limit).await
  }

  /// Sends an authenticated JSON POST and deserializes the JSON response.
  #[tracing::instrument(skip(self, body, token))]
  pub async fn post_json<B: Serialize, T: DeserializeOwned>(
    &self,
    url: &str,
    body: &B,
    token: &str,
  ) -> Result<T, Error> {
    self.rate_limit.check().await;
    tracing::trace!(method = "POST", url = url, "esi: request");
    let start = Instant::now();
    let resp = self.inner.post(url).bearer_auth(token).json(body).send().await?;
    self.rate_limit.update_from_response(&resp);
    let status = resp.status().as_u16();
    tracing::trace!(
      method = "POST",
      url = url,
      status = status,
      elapsed_ms = start.elapsed().as_millis() as u64,
      "esi: response"
    );
    consume_json_response(resp, &self.rate_limit).await
  }

  /// Sends an unauthenticated JSON POST and deserializes the JSON response.
  #[tracing::instrument(skip(self, body))]
  pub async fn post_json_anon<B: Serialize, T: DeserializeOwned>(&self, url: &str, body: &B) -> Result<T, Error> {
    self.rate_limit.check().await;
    tracing::trace!(method = "POST", url = url, "esi: request");
    let start = Instant::now();
    let resp = self.inner.post(url).json(body).send().await?;
    self.rate_limit.update_from_response(&resp);
    let status = resp.status().as_u16();
    tracing::trace!(
      method = "POST",
      url = url,
      status = status,
      elapsed_ms = start.elapsed().as_millis() as u64,
      "esi: response"
    );
    consume_json_response(resp, &self.rate_limit).await
  }

  /// Sends an authenticated JSON PUT and discards the response body.
  #[tracing::instrument(skip(self, body, token))]
  pub async fn put_empty<B: Serialize>(&self, url: &str, body: &B, token: &str) -> Result<(), Error> {
    self.rate_limit.check().await;
    tracing::trace!(method = "PUT", url = url, "esi: request");
    let start = Instant::now();
    let resp = self.inner.put(url).bearer_auth(token).json(body).send().await?;
    self.rate_limit.update_from_response(&resp);
    let status = resp.status().as_u16();
    tracing::trace!(
      method = "PUT",
      url = url,
      status = status,
      elapsed_ms = start.elapsed().as_millis() as u64,
      "esi: response"
    );
    if (200u16..300).contains(&status) {
      return Ok(());
    }
    Err(self.map_error_status(status, resp).await)
  }

  /// Reads the `ETag` from `resp`, stores the body in the cache under `url`, and
  /// deserializes the body as `T`.
  async fn consume_json_with_etag<T: DeserializeOwned>(&self, url: &str, resp: reqwest::Response) -> Result<T, Error> {
    let etag = etag_value(&resp);
    let body = resp.bytes().await?;
    if let Some(tag) = etag {
      self.cache.insert(url, &tag, &body);
    }
    Ok(serde_json::from_slice(&body)?)
  }

  /// Fetches pages 2..=`total_pages` in parallel via a `JoinSet` and returns the merged results.
  async fn fetch_remaining_pages<T: DeserializeOwned + Send + 'static>(
    &self,
    url_base: &str,
    token: Option<&str>,
    total_pages: u32,
  ) -> Result<Vec<T>, Error> {
    let mut set = tokio::task::JoinSet::new();
    for page in 2..=total_pages {
      let http = self.inner.clone();
      let rate_limit = Arc::clone(&self.rate_limit);
      let url = UrlBuilder::new(url_base).param("page", page.to_string()).build();
      let token = token.map(|t| t.to_owned());
      set.spawn(Self::fetch_page(http, rate_limit, url, token));
    }
    let mut results = Vec::new();
    while let Some(result) = set.join_next().await {
      let page = result.map_err(|e| Error::Internal(e.to_string()))??;
      results.extend(page);
    }
    Ok(results)
  }

  /// Maps a non-2xx status code to the appropriate error variant.
  ///
  /// Returns a 420-derived [`Error::RateLimit`] (after sleeping the prescribed
  /// back-off) or an [`Error::Api`] for any other status.
  async fn map_error_status(&self, status: u16, resp: reqwest::Response) -> Error {
    if status == 420 {
      let secs = retry_after_secs(&resp);
      tracing::warn!(status = 420, retry_after_secs = secs, "esi: rate limited");
      self.rate_limit.handle_420(secs).await
    } else {
      api_error(resp).await
    }
  }

  /// Fetches a single page of a paginated ESI endpoint and deserializes it as `Vec<T>`.
  ///
  /// Handles the rate-limit check, optional auth, rate-limit header update, and status
  /// classification. Used by [`Self::get_json_paginated`] to fan out page requests concurrently.
  async fn fetch_page<T: DeserializeOwned>(
    http: reqwest::Client,
    rate_limit: Arc<RateLimiter>,
    url: String,
    token: Option<String>,
  ) -> Result<Vec<T>, Error> {
    rate_limit.check().await;
    let mut req = http.get(&url);
    if let Some(ref t) = token {
      req = req.bearer_auth(t);
    }
    let resp = req.send().await.map_err(Error::from)?;
    rate_limit.update_from_response(&resp);
    consume_json_response(resp, &rate_limit).await
  }
}

/// Token-bucket style rate limiter that respects ESI's `X-ESI-Error-Limit-Remain` headers.
///
/// Call [`check`](RateLimiter::check) before each request; it suspends the task until the
/// window resets if the remaining budget has reached zero.
#[derive(Debug)]
pub(crate) struct RateLimiter {
  state: Mutex<RateLimitState>,
}

impl RateLimiter {
  /// Creates a `RateLimiter` with a default budget of 100 remaining requests.
  pub fn new() -> Self {
    Self {
      state: Mutex::new(RateLimitState {
        remain: 100,
        reset_at: Instant::now(),
      }),
    }
  }

  /// Suspends the current task until the rate-limit window resets, if the budget is exhausted.
  ///
  /// Returns immediately when requests are still available or when the reset instant has
  /// already passed.
  pub async fn check(&self) {
    let sleep = {
      let state = self.state.lock().expect("rate limit mutex poisoned");
      if state.remain == 0 {
        let now = Instant::now();
        if state.reset_at > now {
          Some(state.reset_at - now)
        } else {
          None
        }
      } else {
        None
      }
    };

    if let Some(duration) = sleep {
      tokio::time::sleep(duration).await;
    }
  }

  /// Handles an HTTP 420 response by sleeping for the prescribed back-off period, then
  /// returns an [`Error::RateLimit`] so the caller can propagate it.
  pub async fn handle_420(&self, retry_after_secs: u64) -> Error {
    tokio::time::sleep(Duration::from_secs(retry_after_secs)).await;
    Error::RateLimit {
      retry_after_secs,
    }
  }

  /// Updates the rate-limit state from ESI response headers.
  ///
  /// `remain` maps to `X-ESI-Error-Limit-Remain`; `reset_secs` maps to
  /// `X-ESI-Error-Limit-Reset` expressed as seconds from now.
  pub fn update(&self, remain: u32, reset_secs: u64) {
    let mut state = self.state.lock().expect("rate limit mutex poisoned");
    state.remain = remain;
    state.reset_at = Instant::now() + Duration::from_secs(reset_secs);
  }

  /// Reads ESI error-limit headers from `resp` and updates the rate-limit state.
  pub fn update_from_response(&self, resp: &reqwest::Response) {
    let headers = resp.headers();
    let remain = headers
      .get(ESI_ERROR_REMAIN_HEADER)
      .and_then(|v| v.to_str().ok())
      .and_then(|s| s.parse::<u32>().ok());
    let reset = headers
      .get(ESI_ERROR_RESET_HEADER)
      .and_then(|v| v.to_str().ok())
      .and_then(|s| s.parse::<u64>().ok());
    if let (Some(remain), Some(reset)) = (remain, reset) {
      self.update(remain, reset);
    }
  }
}

/// Fluent builder for constructing ESI endpoint URLs.
///
/// Query parameters are stored in a `BTreeMap` so the serialized query string
/// is deterministically ordered, which simplifies testing and cache-key
/// comparisons.
pub(crate) struct UrlBuilder {
  base: Url,
  path: Option<String>,
  query: BTreeMap<String, String>,
}

impl UrlBuilder {
  /// Creates a new `UrlBuilder` rooted at `base`.
  ///
  /// # Panics
  ///
  /// Panics if `base` is not a valid URL.
  pub fn new(base: &str) -> Self {
    Self {
      base: Url::parse(base).expect("base URL is invalid"),
      path: None,
      query: BTreeMap::new(),
    }
  }

  /// Consumes the builder and returns the fully assembled URL string.
  pub fn build(self) -> String {
    let mut url = self.base;

    if let Some(path) = self.path {
      url.set_path(&format!("/{path}"));
    }

    if !self.query.is_empty() {
      let mut pairs = url.query_pairs_mut();
      for (key, value) in self.query {
        pairs.append_pair(&key, &value);
      }
    }

    url.to_string()
  }

  /// Appends a query parameter to the URL.
  pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
    self.query.insert(key.into(), value.into());
    self
  }

  /// Sets the URL path, stripping any leading `/` to avoid double slashes.
  pub fn path(mut self, path: impl Into<String>) -> Self {
    self.path = Some(path.into().trim_start_matches('/').to_owned());
    self
  }
}

/// Deserializes the `{ "error": "..." }` body returned by ESI on non-2xx responses.
#[derive(Deserialize)]
struct ApiErrorBody {
  /// Human-readable error message from the ESI API.
  error: String,
}

/// Snapshot of the current ESI rate-limit window.
#[derive(Debug)]
struct RateLimitState {
  /// Number of requests remaining in the current window.
  remain: u32,
  /// Instant at which the current window expires and `remain` resets.
  reset_at: Instant,
}

/// Converts a non-2xx response into an [`Error::Api`], reading the error message from the body.
async fn api_error(resp: reqwest::Response) -> Error {
  let status = resp.status().as_u16();
  let error = resp
    .json::<ApiErrorBody>()
    .await
    .map(|b| b.error)
    .unwrap_or_else(|_| "unknown error".to_owned());
  Error::Api {
    error,
    status,
  }
}

/// Classifies `resp` by status code and either deserializes the JSON body as `T` (2xx),
/// sleeps and returns [`Error::RateLimit`] (420), or returns [`Error::Api`] (other).
async fn consume_json_response<T: DeserializeOwned>(
  resp: reqwest::Response,
  rate_limit: &RateLimiter,
) -> Result<T, Error> {
  let status = resp.status().as_u16();
  if (200u16..300).contains(&status) {
    let body = resp.bytes().await?;
    return Ok(serde_json::from_slice(&body)?);
  }
  if status == 420 {
    let secs = retry_after_secs(&resp);
    tracing::warn!(status = 420, retry_after_secs = secs, "esi: rate limited");
    return Err(rate_limit.handle_420(secs).await);
  }
  Err(api_error(resp).await)
}

/// Extracts the `ETag` header value from a response, if present.
fn etag_value(resp: &reqwest::Response) -> Option<String> {
  resp
    .headers()
    .get("ETag")
    .and_then(|v| v.to_str().ok())
    .map(|s| s.to_owned())
}

/// Reads the `X-Pages` header from `resp` and parses it as a `u32`, defaulting to 1.
fn parse_x_pages_header(resp: &reqwest::Response) -> u32 {
  resp
    .headers()
    .get(ESI_PAGES_HEADER)
    .and_then(|v| v.to_str().ok())
    .and_then(|s| s.parse::<u32>().ok())
    .unwrap_or(1)
}

/// Reads the `Retry-After` header and returns its value in seconds, defaulting to 60.
fn retry_after_secs(resp: &reqwest::Response) -> u64 {
  resp
    .headers()
    .get(RETRY_AFTER_HEADER)
    .and_then(|v| v.to_str().ok())
    .and_then(|s| s.parse::<u64>().ok())
    .unwrap_or(60)
}

/// Streams `resp` body chunks to `dest`, writing each chunk as it arrives.
async fn stream_response_to_file(resp: &mut reqwest::Response, dest: &std::path::Path) -> Result<(), Error> {
  use tokio::io::AsyncWriteExt as _;

  let mut file = tokio::fs::File::create(dest)
    .await
    .map_err(|e| Error::Internal(e.to_string()))?;
  while let Some(chunk) = resp.chunk().await? {
    file
      .write_all(&chunk)
      .await
      .map_err(|e| Error::Internal(e.to_string()))?;
  }
  file.shutdown().await.map_err(|e| Error::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
  use super::*;

  mod client {
    use wiremock::MockServer;

    use super::*;
    use crate::cache::MemoryStore;

    fn make_client() -> Client {
      Client {
        cache: CacheStore::Memory(MemoryStore::default()),
        inner: reqwest::Client::new(),
        rate_limit: Arc::new(RateLimiter::new()),
      }
    }

    mod download_to_file {
      use wiremock::{
        Mock, ResponseTemplate,
        matchers::{method, path},
      };

      use super::*;

      #[tokio::test]
      async fn it_writes_response_body_to_file_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/data.bin"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(b"hello bytes".to_vec(), "application/octet-stream"))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/data.bin", server.uri());
        let dest = std::env::temp_dir().join("pod_esi_test_download.bin");

        let result = client.download_to_file(&url, &dest, 30).await;

        assert!(result.is_ok());
        let written = std::fs::read(&dest).unwrap();
        let _ = std::fs::remove_file(&dest);
        assert_eq!(written, b"hello bytes");
      }

      #[tokio::test]
      async fn it_returns_api_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/data.bin"))
          .respond_with(ResponseTemplate::new(404).set_body_raw(r#"{"error":"Not Found"}"#, "application/json"))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/data.bin", server.uri());
        let dest = std::env::temp_dir().join("pod_esi_test_download_404.bin");

        let result = client.download_to_file(&url, &dest, 30).await;

        assert!(matches!(
          result,
          Err(Error::Api {
            status: 404,
            ..
          })
        ));
      }
    }

    mod get_bytes {
      use pretty_assertions::assert_eq;
      use wiremock::{
        Mock, ResponseTemplate,
        matchers::{method, path},
      };

      use super::*;

      #[tokio::test]
      async fn it_returns_body_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/raw"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(b"raw data".to_vec(), "application/octet-stream"))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/raw", server.uri());

        let result = client.get_bytes(&url).await.unwrap();

        assert_eq!(result, b"raw data");
      }

      #[tokio::test]
      async fn it_returns_api_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/raw"))
          .respond_with(ResponseTemplate::new(404).set_body_raw(r#"{"error":"Not Found"}"#, "application/json"))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/raw", server.uri());

        let result = client.get_bytes(&url).await;

        assert!(matches!(
          result,
          Err(Error::Api {
            status: 404,
            ..
          })
        ));
      }

      #[tokio::test]
      async fn it_returns_api_error_on_5xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/raw"))
          .respond_with(ResponseTemplate::new(503).set_body_raw(r#"{"error":"Unavailable"}"#, "application/json"))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/raw", server.uri());

        let result = client.get_bytes(&url).await;

        assert!(matches!(
          result,
          Err(Error::Api {
            status: 503,
            ..
          })
        ));
      }
    }

    mod post_json_anon {
      use pretty_assertions::assert_eq;
      use wiremock::{
        Mock, ResponseTemplate,
        matchers::{method, path},
      };

      use super::*;

      #[tokio::test]
      async fn it_returns_api_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/anon"))
          .respond_with(ResponseTemplate::new(401).set_body_raw(r#"{"error":"Unauthorized"}"#, "application/json"))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/anon", server.uri());

        let result: Result<serde_json::Value, _> =
          client.post_json_anon(&url, &serde_json::json!({"key": "val"})).await;

        assert!(matches!(
          result,
          Err(Error::Api {
            status: 401,
            ..
          })
        ));
      }

      #[tokio::test]
      async fn it_returns_deserialized_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/anon"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"result":"ok"}"#, "application/json"))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/anon", server.uri());

        let result: serde_json::Value = client
          .post_json_anon(&url, &serde_json::json!({"key": "val"}))
          .await
          .unwrap();

        assert_eq!(result["result"], "ok");
      }
    }

    mod delete_empty {
      use wiremock::{
        Mock, ResponseTemplate,
        matchers::{method, path},
      };

      use super::*;

      #[tokio::test]
      async fn it_returns_api_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
          .and(path("/things/1"))
          .respond_with(ResponseTemplate::new(403).set_body_raw(r#"{"error":"Forbidden"}"#, "application/json"))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/things/1", server.uri());

        let result = client.delete_empty(&url, "token").await;

        assert!(matches!(
          result,
          Err(Error::Api {
            status: 403,
            ..
          })
        ));
      }

      #[tokio::test]
      async fn it_returns_ok_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
          .and(path("/things/1"))
          .respond_with(ResponseTemplate::new(204))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/things/1", server.uri());

        let result = client.delete_empty(&url, "token").await;

        assert!(result.is_ok());
      }
    }

    mod get_json {
      use bytes::Bytes;
      use pretty_assertions::assert_eq;
      use wiremock::{
        Mock, ResponseTemplate,
        matchers::{method, path},
      };

      use super::*;

      #[tokio::test]
      async fn it_fetches_and_caches_on_200() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
          .and(path("/items"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("ETag", "\"xyz\"")
              .set_body_raw("[4,5,6]", "application/json"),
          )
          .mount(&server)
          .await;

        let url = format!("{}/items", server.uri());
        let client = Client {
          cache: CacheStore::Memory(MemoryStore::default()),
          inner: reqwest::Client::new(),
          rate_limit: Arc::new(RateLimiter::new()),
        };

        let result: Vec<i32> = client.get_json(&url, None).await.unwrap();

        assert_eq!(result, vec![4, 5, 6]);
      }

      #[tokio::test]
      async fn it_returns_api_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/items"))
          .respond_with(ResponseTemplate::new(404).set_body_raw(r#"{"error":"Not Found"}"#, "application/json"))
          .mount(&server)
          .await;
        let url = format!("{}/items", server.uri());
        let client = make_client();

        let result: Result<Vec<i32>, _> = client.get_json(&url, None).await;

        assert!(matches!(
          result,
          Err(Error::Api {
            status: 404,
            ..
          })
        ));
      }

      #[tokio::test]
      async fn it_returns_cached_body_on_304() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
          .and(path("/items"))
          .respond_with(ResponseTemplate::new(304))
          .mount(&server)
          .await;

        let cache = CacheStore::Memory(MemoryStore::default());
        let url = format!("{}/items", server.uri());
        cache.insert(&url, "\"abc\"", &Bytes::from("[1,2,3]"));
        let client = Client {
          cache,
          inner: reqwest::Client::new(),
          rate_limit: Arc::new(RateLimiter::new()),
        };

        let result: Vec<i32> = client.get_json(&url, None).await.unwrap();

        assert_eq!(result, vec![1, 2, 3]);
      }
    }

    mod get_json_paginated {
      use pretty_assertions::assert_eq;
      use wiremock::{
        Mock, ResponseTemplate,
        matchers::{method, path, query_param},
      };

      use super::*;

      #[tokio::test]
      async fn it_merges_all_pages() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
          .and(path("/items"))
          .and(query_param("page", "1"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "3")
              .set_body_raw("[1,2]", "application/json"),
          )
          .mount(&server)
          .await;

        Mock::given(method("GET"))
          .and(path("/items"))
          .and(query_param("page", "2"))
          .respond_with(ResponseTemplate::new(200).set_body_raw("[3,4]", "application/json"))
          .mount(&server)
          .await;

        Mock::given(method("GET"))
          .and(path("/items"))
          .and(query_param("page", "3"))
          .respond_with(ResponseTemplate::new(200).set_body_raw("[5,6]", "application/json"))
          .mount(&server)
          .await;

        let client = make_client();
        let result: Vec<i32> = client
          .get_json_paginated(&format!("{}/items", server.uri()), None)
          .await
          .unwrap();

        let mut sorted = result;
        sorted.sort();
        assert_eq!(sorted, vec![1, 2, 3, 4, 5, 6]);
      }

      #[tokio::test]
      async fn it_returns_api_error_on_4xx_for_page_1() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/items"))
          .and(query_param("page", "1"))
          .respond_with(ResponseTemplate::new(403).set_body_raw(r#"{"error":"Forbidden"}"#, "application/json"))
          .mount(&server)
          .await;
        let client = make_client();

        let result: Result<Vec<i32>, _> = client
          .get_json_paginated(&format!("{}/items", server.uri()), None)
          .await;

        assert!(matches!(
          result,
          Err(Error::Api {
            status: 403,
            ..
          })
        ));
      }

      #[tokio::test]
      async fn it_returns_single_page_when_no_x_pages_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/items"))
          .and(query_param("page", "1"))
          .respond_with(ResponseTemplate::new(200).set_body_raw("[7,8,9]", "application/json"))
          .mount(&server)
          .await;
        let client = make_client();

        let result: Vec<i32> = client
          .get_json_paginated(&format!("{}/items", server.uri()), None)
          .await
          .unwrap();

        assert_eq!(result, vec![7, 8, 9]);
      }

      #[tokio::test]
      async fn it_sends_bearer_token_when_provided() {
        use wiremock::matchers::header;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/items"))
          .and(query_param("page", "1"))
          .and(header("Authorization", "Bearer my-token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw("[1]", "application/json"))
          .mount(&server)
          .await;
        let client = make_client();

        let result: Vec<i32> = client
          .get_json_paginated(&format!("{}/items", server.uri()), Some("my-token"))
          .await
          .unwrap();

        assert_eq!(result, vec![1]);
      }
    }

    mod post_empty {
      use wiremock::{
        Mock, ResponseTemplate,
        matchers::{method, path},
      };

      use super::*;

      #[tokio::test]
      async fn it_returns_api_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/things"))
          .respond_with(ResponseTemplate::new(422).set_body_raw(r#"{"error":"Unprocessable"}"#, "application/json"))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/things", server.uri());

        let result = client.post_empty(&url, &serde_json::Value::Null, "token").await;

        assert!(matches!(
          result,
          Err(Error::Api {
            status: 422,
            ..
          })
        ));
      }

      #[tokio::test]
      async fn it_returns_ok_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/things"))
          .respond_with(ResponseTemplate::new(204))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/things", server.uri());

        let result = client.post_empty(&url, &serde_json::Value::Null, "token").await;

        assert!(result.is_ok());
      }
    }

    mod post_form_anon {
      use pretty_assertions::assert_eq;
      use wiremock::{
        Mock, ResponseTemplate,
        matchers::{method, path},
      };

      use super::*;

      #[tokio::test]
      async fn it_returns_api_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/token"))
          .respond_with(ResponseTemplate::new(400).set_body_raw(r#"{"error":"Bad Request"}"#, "application/json"))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/token", server.uri());
        let body = [("grant_type", "authorization_code")];

        let result: Result<serde_json::Value, _> = client.post_form_anon(&url, &body).await;

        assert!(matches!(
          result,
          Err(Error::Api {
            status: 400,
            ..
          })
        ));
      }

      #[tokio::test]
      async fn it_returns_deserialized_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/token"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"access_token":"abc"}"#, "application/json"))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/token", server.uri());
        let body = [("grant_type", "authorization_code")];

        let result: serde_json::Value = client.post_form_anon(&url, &body).await.unwrap();

        assert_eq!(result["access_token"], "abc");
      }
    }

    mod post_json {
      use pretty_assertions::assert_eq;
      use wiremock::{
        Mock, ResponseTemplate,
        matchers::{method, path},
      };

      use super::*;

      #[tokio::test]
      async fn it_returns_api_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/items"))
          .respond_with(ResponseTemplate::new(422).set_body_raw(r#"{"error":"Unprocessable"}"#, "application/json"))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/items", server.uri());

        let result: Result<serde_json::Value, _> =
          client.post_json(&url, &serde_json::json!({"value": 1}), "token").await;

        assert!(matches!(
          result,
          Err(Error::Api {
            status: 422,
            ..
          })
        ));
      }

      #[tokio::test]
      async fn it_returns_deserialized_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/items"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"id":42}"#, "application/json"))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/items", server.uri());

        let result: serde_json::Value = client
          .post_json(&url, &serde_json::json!({"value": 1}), "token")
          .await
          .unwrap();

        assert_eq!(result["id"], 42);
      }
    }

    mod put_empty {
      use wiremock::{
        Mock, ResponseTemplate,
        matchers::{method, path},
      };

      use super::*;

      #[tokio::test]
      async fn it_returns_api_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
          .and(path("/things/1"))
          .respond_with(ResponseTemplate::new(403).set_body_raw(r#"{"error":"Forbidden"}"#, "application/json"))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/things/1", server.uri());

        let result = client.put_empty(&url, &serde_json::json!({"value": 1}), "token").await;

        assert!(matches!(
          result,
          Err(Error::Api {
            status: 403,
            ..
          })
        ));
      }

      #[tokio::test]
      async fn it_returns_ok_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
          .and(path("/things/1"))
          .respond_with(ResponseTemplate::new(200))
          .mount(&server)
          .await;
        let client = make_client();
        let url = format!("{}/things/1", server.uri());

        let result = client.put_empty(&url, &serde_json::json!({"value": 1}), "token").await;

        assert!(result.is_ok());
      }
    }
  }

  mod consume_json_response {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;

    fn make_rate_limiter() -> Arc<RateLimiter> {
      Arc::new(RateLimiter::new())
    }

    #[tokio::test]
    async fn it_deserializes_body_on_2xx() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"value":7}"#, "application/json"))
        .mount(&server)
        .await;
      let rl = make_rate_limiter();
      let resp = reqwest::Client::new()
        .get(format!("{}/data", server.uri()))
        .send()
        .await
        .unwrap();

      let result: serde_json::Value = consume_json_response(resp, &rl).await.unwrap();

      assert_eq!(result["value"], 7);
    }

    #[tokio::test]
    async fn it_returns_api_error_on_4xx() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(404).set_body_raw(r#"{"error":"Not Found"}"#, "application/json"))
        .mount(&server)
        .await;
      let rl = make_rate_limiter();
      let resp = reqwest::Client::new()
        .get(format!("{}/data", server.uri()))
        .send()
        .await
        .unwrap();

      let result: Result<serde_json::Value, _> = consume_json_response(resp, &rl).await;

      assert!(matches!(
        result,
        Err(Error::Api {
          status: 404,
          ..
        })
      ));
    }

    #[tokio::test]
    async fn it_returns_api_error_on_5xx() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(ResponseTemplate::new(500).set_body_raw(r#"{"error":"Server Error"}"#, "application/json"))
        .mount(&server)
        .await;
      let rl = make_rate_limiter();
      let resp = reqwest::Client::new()
        .get(format!("{}/data", server.uri()))
        .send()
        .await
        .unwrap();

      let result: Result<serde_json::Value, _> = consume_json_response(resp, &rl).await;

      assert!(matches!(
        result,
        Err(Error::Api {
          status: 500,
          ..
        })
      ));
    }
  }

  mod parse_x_pages_header {
    use pretty_assertions::assert_eq;
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

    use super::*;

    async fn get_response_with_header(header: Option<&str>) -> reqwest::Response {
      let server = MockServer::start().await;
      let mut template = ResponseTemplate::new(200).set_body_raw("[]", "application/json");
      if let Some(v) = header {
        template = template.insert_header("X-Pages", v);
      }
      Mock::given(method("GET")).respond_with(template).mount(&server).await;
      reqwest::Client::new().get(server.uri()).send().await.unwrap()
    }

    #[tokio::test]
    async fn it_returns_parsed_value_when_header_is_present() {
      let resp = get_response_with_header(Some("5")).await;

      assert_eq!(parse_x_pages_header(&resp), 5);
    }

    #[tokio::test]
    async fn it_returns_one_when_header_is_absent() {
      let resp = get_response_with_header(None).await;

      assert_eq!(parse_x_pages_header(&resp), 1);
    }

    #[tokio::test]
    async fn it_returns_one_when_header_is_not_numeric() {
      let resp = get_response_with_header(Some("abc")).await;

      assert_eq!(parse_x_pages_header(&resp), 1);
    }
  }

  mod rate_limiter {
    use super::*;

    mod check {
      use super::*;

      #[tokio::test]
      async fn it_returns_immediately_when_budget_is_not_exhausted() {
        let limiter = RateLimiter::new();

        let start = Instant::now();
        limiter.check().await;

        assert!(start.elapsed() < Duration::from_millis(50));
      }

      #[tokio::test]
      async fn it_returns_immediately_when_budget_is_zero_and_reset_has_passed() {
        let limiter = RateLimiter::new();
        limiter.update(0, 0);

        let start = Instant::now();
        limiter.check().await;

        assert!(start.elapsed() < Duration::from_millis(50));
      }
    }

    mod handle_420 {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_returns_rate_limit_error_with_correct_seconds() {
        let limiter = RateLimiter::new();
        let err = limiter.handle_420(0).await;

        assert_eq!(err.to_string(), "rate limited; retry after 0s");
      }
    }

    mod update {
      use super::*;

      #[test]
      fn it_updates_remain_and_reset_at() {
        let limiter = RateLimiter::new();
        limiter.update(42, 10);

        let state = limiter.state.lock().unwrap();
        assert_eq!(state.remain, 42);
        assert!(state.reset_at > Instant::now());
      }
    }

    mod update_from_response {
      use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

      use super::*;

      #[tokio::test]
      async fn it_updates_state_when_headers_are_present() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header(ESI_ERROR_REMAIN_HEADER, "50")
              .insert_header(ESI_ERROR_RESET_HEADER, "30")
              .set_body_raw("[]", "application/json"),
          )
          .mount(&server)
          .await;
        let limiter = Arc::new(RateLimiter::new());
        let resp = reqwest::Client::new().get(server.uri()).send().await.unwrap();

        limiter.update_from_response(&resp);

        let state = limiter.state.lock().unwrap();
        assert_eq!(state.remain, 50);
      }

      #[tokio::test]
      async fn it_does_not_update_when_headers_are_absent() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
          .mount(&server)
          .await;
        let limiter = Arc::new(RateLimiter::new());
        let resp = reqwest::Client::new().get(server.uri()).send().await.unwrap();

        limiter.update_from_response(&resp);

        let state = limiter.state.lock().unwrap();
        assert_eq!(state.remain, 100);
      }
    }
  }

  mod url_builder {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_builds_a_url_with_only_a_path() {
      let url = UrlBuilder::new("https://www.test.com").path("/test").build();

      assert_eq!(url, "https://www.test.com/test");
    }

    #[test]
    fn it_builds_a_url_with_query_parameters() {
      let url = UrlBuilder::new("https://www.test.com")
        .path("/test")
        .param("key1", "value1")
        .param("key2", "value2")
        .build();

      assert_eq!(url, "https://www.test.com/test?key1=value1&key2=value2");
    }

    #[test]
    fn it_percent_encodes_query_parameters() {
      let url = UrlBuilder::new("https://www.test.com")
        .path("/test")
        .param("q", "hello world")
        .build();

      assert_eq!(url, "https://www.test.com/test?q=hello+world");
    }
  }
}
