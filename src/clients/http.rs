use std::{
  collections::{HashMap, VecDeque},
  sync::Arc,
  time::{Duration, Instant},
};

use chrono::Utc;
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::Mutex;

use crate::{
  clients::Error,
  store::{self, model::HttpCacheEntry, repo::infra},
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const ESI_ERROR_LIMIT_RESET_HEADER: &str = "X-ESI-Error-Limit-Reset";
const ESI_PAGES_HEADER: &str = "X-Pages";
const HTTP_TARGET: &str = "pod::http";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Cache {
  db: store::Database,
}

impl Cache {
  pub fn new(db: store::Database) -> Self {
    Self {
      db,
    }
  }

  async fn get(&self, url: &str) -> Result<Option<HttpCacheEntry>, store::Error> {
    infra::http_cache_get(&self.db, url).await
  }

  async fn upsert(&self, entry: &HttpCacheEntry) -> Result<(), store::Error> {
    infra::http_cache_upsert(&self.db, entry).await
  }
}

pub struct Client {
  cache: Cache,
  inner: reqwest::Client,
  rate_limiters: Arc<HashMap<String, Mutex<RateLimiter>>>,
}

impl Client {
  pub fn builder(cache: Cache) -> ClientBuilder {
    ClientBuilder {
      cache,
      rate_limiters: HashMap::new(),
    }
  }

  #[allow(dead_code)]
  pub async fn delete_empty(&self, url: &str, token: &str) -> Result<(), Error> {
    self.apply_rate_limit(url).await;
    let resp = send_logged("DELETE", url, self.inner.delete(url).bearer_auth(token)).await?;
    handle_status(resp).await
  }

  #[allow(dead_code)]
  pub async fn get_bytes(&self, url: &str, token: Option<&str>) -> Result<Vec<u8>, Error> {
    self.get_cached_bytes(url, token, true).await
  }

  pub async fn get_bytes_uncached(&self, url: &str) -> Result<Vec<u8>, Error> {
    self.apply_rate_limit(url).await;
    let resp = send_logged("GET", url, self.inner.get(url)).await?;
    if let Some(err) = throttle_error(&resp) {
      return Err(err);
    }
    if !(200..300).contains(&resp.status().as_u16()) {
      return Err(Error::Http(resp.error_for_status().unwrap_err()));
    }
    Ok(resp.bytes().await?.to_vec())
  }

  pub async fn get_json<T: DeserializeOwned>(&self, url: &str, token: Option<&str>) -> Result<T, Error> {
    let body = self.get_cached_bytes(url, token, false).await?;
    Ok(serde_json::from_slice(&body)?)
  }

  pub async fn get_json_paginated<T: DeserializeOwned + Send + 'static>(
    &self,
    url: &str,
    token: Option<&str>,
  ) -> Result<Vec<T>, Error> {
    let (mut items, total_pages) = fetch_page::<T>(&self.inner, &self.rate_limiters, url, 1, token).await?;

    if total_pages <= 1 {
      return Ok(items);
    }

    let mut set = tokio::task::JoinSet::new();
    for page in 2..=total_pages {
      let inner = self.inner.clone();
      let rate_limiters = Arc::clone(&self.rate_limiters);
      let url = url.to_owned();
      let token = token.map(str::to_owned);
      set.spawn(async move {
        fetch_page::<T>(&inner, &rate_limiters, &url, page, token.as_deref())
          .await
          .map(|(items, _)| items)
      });
    }

    while let Some(joined) = set.join_next().await {
      let page_items = joined.map_err(|e| Error::Internal(format!("page task panicked: {e}")))??;
      items.extend(page_items);
    }

    Ok(items)
  }

  #[allow(dead_code)]
  pub async fn post_empty<B: Serialize>(&self, url: &str, body: &B, token: &str) -> Result<(), Error> {
    self.apply_rate_limit(url).await;
    let resp = send_logged("POST", url, self.inner.post(url).bearer_auth(token).json(body)).await?;
    handle_status(resp).await
  }

  pub async fn post_form<B: Serialize, T: DeserializeOwned>(&self, url: &str, body: &B) -> Result<T, Error> {
    self.apply_rate_limit(url).await;
    let resp = send_logged("POST", url, self.inner.post(url).form(body)).await?;
    deserialize_response(resp).await
  }

  pub async fn post_json<B: Serialize, T: DeserializeOwned>(
    &self,
    url: &str,
    body: &B,
    token: &str,
  ) -> Result<T, Error> {
    self.apply_rate_limit(url).await;
    let resp = send_logged("POST", url, self.inner.post(url).bearer_auth(token).json(body)).await?;
    deserialize_response(resp).await
  }

  pub async fn post_json_anon<B: Serialize, T: DeserializeOwned>(&self, url: &str, body: &B) -> Result<T, Error> {
    self.apply_rate_limit(url).await;
    let resp = send_logged("POST", url, self.inner.post(url).json(body)).await?;
    deserialize_response(resp).await
  }

  pub async fn put_empty<B: Serialize>(&self, url: &str, body: &B, token: &str) -> Result<(), Error> {
    self.apply_rate_limit(url).await;
    let resp = send_logged("PUT", url, self.inner.put(url).bearer_auth(token).json(body)).await?;
    handle_status(resp).await
  }

  async fn apply_rate_limit(&self, url: &str) {
    enforce_rate_limit(&self.rate_limiters, url).await;
  }

  async fn get_cached_bytes(&self, url: &str, token: Option<&str>, serve_fresh: bool) -> Result<Vec<u8>, Error> {
    self.apply_rate_limit(url).await;
    let cached = self.cache.get(url).await?;

    if serve_fresh
      && let Some(ref entry) = cached
      && !entry.is_expired()
    {
      tracing::debug!(target: HTTP_TARGET, method = "GET", url, cache = "hit", "served from fresh cache");
      return Ok(entry.body().to_vec());
    }

    let mut req = self.inner.get(url);
    if let Some(ref entry) = cached
      && let Some(etag) = entry.etag()
    {
      req = req.header("If-None-Match", etag.as_str());
    }
    if let Some(t) = token {
      req = req.bearer_auth(t);
    }

    let resp = send_logged("GET", url, req).await?;
    let status = resp.status().as_u16();

    if status == 304 {
      tracing::debug!(target: HTTP_TARGET, method = "GET", url, status, cache = "not-modified", "revalidated; served from cache");
      let entry = cached.expect("304 requires a prior cached entry");
      return Ok(entry.body().to_vec());
    }
    if let Some(err) = throttle_error(&resp) {
      return Err(err);
    }
    if !(200..300).contains(&status) {
      return Err(Error::Http(resp.error_for_status().unwrap_err()));
    }

    let etag = resp
      .headers()
      .get("ETag")
      .and_then(|v| v.to_str().ok())
      .map(|s| s.to_owned());
    let expires_at = expires_at_from_response(&resp);
    let body = resp.bytes().await?;

    let mut entry = HttpCacheEntry::new(body.to_vec(), Utc::now().timestamp(), url);
    if let Some(tag) = etag {
      entry.set_etag(tag);
    }
    if let Some(exp) = expires_at {
      entry.set_expires_at(exp);
    }
    self.cache.upsert(&entry).await?;

    Ok(body.to_vec())
  }
}

pub struct ClientBuilder {
  cache: Cache,
  rate_limiters: HashMap<String, Mutex<RateLimiter>>,
}

impl ClientBuilder {
  pub fn build(self) -> Arc<Client> {
    let inner = reqwest::Client::builder()
      .user_agent(crate::clients::user_agent())
      .connect_timeout(CONNECT_TIMEOUT)
      .timeout(REQUEST_TIMEOUT)
      .build()
      .expect("failed to build reqwest client");
    Arc::new(Client {
      cache: self.cache,
      inner,
      rate_limiters: Arc::new(self.rate_limiters),
    })
  }

  #[allow(dead_code)]
  pub fn rate_limit(mut self, prefix: &str, max_requests: u32, window: Duration) -> Self {
    self
      .rate_limiters
      .insert(prefix.to_owned(), Mutex::new(RateLimiter::new(max_requests, window)));
    self
  }
}

pub struct RateLimiter {
  max_requests: u32,
  timestamps: VecDeque<Instant>,
  window: Duration,
}

impl RateLimiter {
  pub fn new(max_requests: u32, window: Duration) -> Self {
    Self {
      max_requests,
      timestamps: VecDeque::new(),
      window,
    }
  }

  pub async fn check(&mut self) {
    loop {
      let now = Instant::now();
      while let Some(&front) = self.timestamps.front() {
        if now.duration_since(front) >= self.window {
          self.timestamps.pop_front();
        } else {
          break;
        }
      }

      if (self.timestamps.len() as u32) < self.max_requests {
        self.timestamps.push_back(now);
        return;
      }

      let oldest = self.timestamps[0];
      let elapsed = now.duration_since(oldest);
      let sleep_for = self.window.saturating_sub(elapsed);
      tokio::time::sleep(sleep_for).await;
    }
  }
}

async fn deserialize_response<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, Error> {
  let status = resp.status().as_u16();
  if let Some(err) = throttle_error(&resp) {
    return Err(err);
  }
  if !(200..300).contains(&status) {
    return Err(Error::Http(resp.error_for_status().unwrap_err()));
  }
  let body = resp.bytes().await?;
  Ok(serde_json::from_slice(&body)?)
}

async fn enforce_rate_limit(rate_limiters: &HashMap<String, Mutex<RateLimiter>>, url: &str) {
  if let Some(limiter) = find_rate_limiter_for(rate_limiters, url) {
    let mut guard = limiter.lock().await;
    guard.check().await;
  }
}

fn expires_at_from_response(resp: &reqwest::Response) -> Option<i64> {
  let cc = resp.headers().get("Cache-Control")?.to_str().ok()?;
  for directive in cc.split(',') {
    if let Some(val) = directive.trim().strip_prefix("max-age=") {
      let secs: i64 = val.trim().parse().ok()?;
      return Some(Utc::now().timestamp() + secs);
    }
  }
  None
}

async fn fetch_page<T: DeserializeOwned>(
  inner: &reqwest::Client,
  rate_limiters: &HashMap<String, Mutex<RateLimiter>>,
  url_base: &str,
  page: u32,
  token: Option<&str>,
) -> Result<(Vec<T>, u32), Error> {
  enforce_rate_limit(rate_limiters, url_base).await;

  let separator = if url_base.contains('?') { '&' } else { '?' };
  let url = format!("{url_base}{separator}page={page}");
  let mut req = inner.get(&url);
  if let Some(t) = token {
    req = req.bearer_auth(t);
  }

  let resp = send_logged("GET", &url, req).await?;
  let status = resp.status().as_u16();
  if let Some(err) = throttle_error(&resp) {
    return Err(err);
  }
  if !(200..300).contains(&status) {
    return Err(Error::Http(resp.error_for_status().unwrap_err()));
  }

  let total_pages = parse_x_pages(&resp);
  let body = resp.bytes().await?;
  let items = serde_json::from_slice(&body)?;

  Ok((items, total_pages))
}

fn find_rate_limiter_for<'a>(
  rate_limiters: &'a HashMap<String, Mutex<RateLimiter>>,
  url: &str,
) -> Option<&'a Mutex<RateLimiter>> {
  let path = url_path(url);
  rate_limiters
    .iter()
    .filter(|(prefix, _)| path.starts_with(prefix.as_str()))
    .max_by_key(|(prefix, _)| prefix.len())
    .map(|(_, limiter)| limiter)
}

async fn handle_status(resp: reqwest::Response) -> Result<(), Error> {
  let status = resp.status().as_u16();
  if let Some(err) = throttle_error(&resp) {
    return Err(err);
  }
  if !(200..300).contains(&status) {
    return Err(Error::Http(resp.error_for_status().unwrap_err()));
  }
  Ok(())
}

fn parse_error_limit_reset(resp: &reqwest::Response) -> u64 {
  resp
    .headers()
    .get(ESI_ERROR_LIMIT_RESET_HEADER)
    .and_then(|v| v.to_str().ok())
    .and_then(|s| s.parse::<u64>().ok())
    .unwrap_or(60)
}

fn parse_retry_after(resp: &reqwest::Response) -> u64 {
  resp
    .headers()
    .get("Retry-After")
    .and_then(|v| v.to_str().ok())
    .and_then(|s| s.parse::<u64>().ok())
    .unwrap_or(60)
}

fn parse_x_pages(resp: &reqwest::Response) -> u32 {
  resp
    .headers()
    .get(ESI_PAGES_HEADER)
    .and_then(|v| v.to_str().ok())
    .and_then(|s| s.parse::<u32>().ok())
    .unwrap_or(1)
}

async fn send_logged(method: &str, url: &str, req: reqwest::RequestBuilder) -> Result<reqwest::Response, Error> {
  let started = Instant::now();
  let result = req.send().await;
  let elapsed_ms = started.elapsed().as_millis() as u64;

  match result {
    Ok(resp) => {
      let status = resp.status().as_u16();
      if (200..400).contains(&status) {
        tracing::debug!(target: HTTP_TARGET, method, url, status, elapsed_ms, "request completed");
      } else {
        tracing::warn!(target: HTTP_TARGET, method, url, status, elapsed_ms, "request returned non-success status");
      }
      Ok(resp)
    }
    Err(error) => {
      tracing::error!(target: HTTP_TARGET, method, url, elapsed_ms, %error, "request failed");
      Err(error.into())
    }
  }
}

fn throttle_error(resp: &reqwest::Response) -> Option<Error> {
  match resp.status().as_u16() {
    420 => Some(Error::ErrorLimited {
      reset_secs: parse_error_limit_reset(resp),
    }),
    429 => Some(Error::RateLimit {
      retry_after_secs: parse_retry_after(resp),
    }),
    _ => None,
  }
}

fn url_path(url: &str) -> &str {
  url
    .find("://")
    .and_then(|i| url[i + 3..].find('/').map(|j| &url[i + 3 + j..]))
    .map(|s| &s[..s.find('?').or_else(|| s.find('#')).unwrap_or(s.len())])
    .unwrap_or("/")
}

#[cfg(test)]
mod tests {
  use super::*;

  mod client_builder {
    use super::*;

    mod build {
      use super::*;

      #[tokio::test]
      async fn it_succeeds_with_no_rate_limits() {
        let db = store::open_test().await.unwrap();
        let cache = Cache::new(db);

        let _client = Client::builder(cache).build();
      }
    }
  }

  mod client {
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{header, method, path, query_param},
    };

    use super::*;
    use crate::store::{self, repo::infra};

    async fn make_test_client() -> (Arc<Client>, store::Database) {
      let db = store::open_test().await.unwrap();
      let cache = Cache::new(db.clone());
      let client = Client::builder(cache).build();
      (client, db)
    }

    mod delete_empty {
      use super::*;

      #[tokio::test]
      async fn it_returns_http_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
          .and(path("/things/1"))
          .respond_with(ResponseTemplate::new(403))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result = client
          .delete_empty(&format!("{}/things/1", server.uri()), "token")
          .await;

        assert!(matches!(result, Err(Error::Http(_))));
      }

      #[tokio::test]
      async fn it_returns_ok_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
          .and(path("/things/1"))
          .respond_with(ResponseTemplate::new(204))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result = client
          .delete_empty(&format!("{}/things/1", server.uri()), "token")
          .await;

        assert!(result.is_ok());
      }

      #[tokio::test]
      async fn it_returns_rate_limit_error_on_429() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
          .and(path("/things/1"))
          .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "30"))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result = client
          .delete_empty(&format!("{}/things/1", server.uri()), "token")
          .await;

        assert!(matches!(
          result,
          Err(Error::RateLimit {
            retry_after_secs: 30
          })
        ));
      }
    }

    mod get_bytes {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_returns_and_caches_the_body_on_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/portrait"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("ETag", "\"v1\"")
              .set_body_raw(vec![1u8, 2, 3], "image/png"),
          )
          .mount(&server)
          .await;
        let (client, db) = make_test_client().await;
        let url = format!("{}/portrait", server.uri());

        let bytes = client.get_bytes(&url, None).await.unwrap();

        assert_eq!(bytes, vec![1u8, 2, 3]);
        let cached = infra::http_cache_get(&db, &url).await.unwrap().unwrap();
        assert_eq!(cached.body(), &[1u8, 2, 3]);
      }

      #[tokio::test]
      async fn it_serves_a_fresh_cached_body_without_a_request() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/portrait"))
          .respond_with(ResponseTemplate::new(500))
          .expect(0)
          .mount(&server)
          .await;
        let (client, db) = make_test_client().await;
        let url = format!("{}/portrait", server.uri());
        let mut entry = HttpCacheEntry::new(vec![9u8, 9, 9], 0, &url);
        entry.set_expires_at(Utc::now().timestamp() + 3600);
        infra::http_cache_upsert(&db, &entry).await.unwrap();

        let bytes = client.get_bytes(&url, None).await.unwrap();

        assert_eq!(bytes, vec![9u8, 9, 9]);
      }
    }

    mod get_json {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_returns_cached_body_on_304() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/resource"))
          .respond_with(ResponseTemplate::new(304))
          .mount(&server)
          .await;
        let db = store::open_test().await.unwrap();
        let url = format!("{}/resource", server.uri());
        let mut entry = HttpCacheEntry::new(b"[7,8,9]".to_vec(), 0, &url);
        entry.set_etag("\"abc\"");
        infra::http_cache_upsert(&db, &entry).await.unwrap();
        let client = Client::builder(Cache::new(db)).build();

        let result: Vec<i32> = client.get_json(&url, None).await.unwrap();

        assert_eq!(result, vec![7, 8, 9]);
      }

      #[tokio::test]
      async fn it_returns_http_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/resource"))
          .respond_with(ResponseTemplate::new(404))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result: Result<Vec<i32>, _> = client.get_json(&format!("{}/resource", server.uri()), None).await;

        assert!(matches!(result, Err(Error::Http(_))));
      }

      #[tokio::test]
      async fn it_returns_rate_limit_error_on_429() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/resource"))
          .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "60"))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result: Result<Vec<i32>, _> = client.get_json(&format!("{}/resource", server.uri()), None).await;

        assert!(matches!(
          result,
          Err(Error::RateLimit {
            retry_after_secs: 60
          })
        ));
      }

      #[tokio::test]
      async fn it_returns_error_limited_on_420() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/resource"))
          .respond_with(ResponseTemplate::new(420).insert_header("X-ESI-Error-Limit-Reset", "8"))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result: Result<Vec<i32>, _> = client.get_json(&format!("{}/resource", server.uri()), None).await;

        assert!(matches!(
          result,
          Err(Error::ErrorLimited {
            reset_secs: 8
          })
        ));
      }

      #[tokio::test]
      async fn it_sends_if_none_match_when_etag_is_cached() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/resource"))
          .and(header("If-None-Match", "\"cached-etag\""))
          .respond_with(ResponseTemplate::new(304))
          .mount(&server)
          .await;
        let db = store::open_test().await.unwrap();
        let url = format!("{}/resource", server.uri());
        let mut entry = HttpCacheEntry::new(b"[1]".to_vec(), 0, &url);
        entry.set_etag("\"cached-etag\"");
        infra::http_cache_upsert(&db, &entry).await.unwrap();
        let client = Client::builder(Cache::new(db)).build();

        let result: Vec<i32> = client.get_json(&url, None).await.unwrap();

        assert_eq!(result, vec![1]);
      }

      #[tokio::test]
      async fn it_stores_response_to_cache_on_200_with_etag() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/resource"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("ETag", "\"new-etag\"")
              .set_body_raw(b"[1,2,3]".to_vec(), "application/json"),
          )
          .mount(&server)
          .await;
        let db = store::open_test().await.unwrap();
        let url = format!("{}/resource", server.uri());
        let client = Client::builder(Cache::new(db.clone())).build();

        let _: Vec<i32> = client.get_json(&url, None).await.unwrap();

        let cached = infra::http_cache_get(&db, &url).await.unwrap().unwrap();
        assert_eq!(cached.etag().as_deref(), Some("\"new-etag\""));
        assert_eq!(cached.body(), b"[1,2,3]");
      }
    }

    mod get_json_paginated {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_fetches_and_merges_all_pages() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/list"))
          .and(query_param("page", "1"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "3")
              .set_body_raw(b"[1,2]".to_vec(), "application/json"),
          )
          .mount(&server)
          .await;
        Mock::given(method("GET"))
          .and(path("/list"))
          .and(query_param("page", "2"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "3")
              .set_body_raw(b"[3,4]".to_vec(), "application/json"),
          )
          .mount(&server)
          .await;
        Mock::given(method("GET"))
          .and(path("/list"))
          .and(query_param("page", "3"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "3")
              .set_body_raw(b"[5,6]".to_vec(), "application/json"),
          )
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let mut result: Vec<i32> = client
          .get_json_paginated(&format!("{}/list", server.uri()), None)
          .await
          .unwrap();
        result.sort();

        assert_eq!(result, vec![1, 2, 3, 4, 5, 6]);
      }

      #[tokio::test]
      async fn it_returns_single_page_without_fanning_out() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/list"))
          .and(query_param("page", "1"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "1")
              .set_body_raw(b"[9]".to_vec(), "application/json"),
          )
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result: Vec<i32> = client
          .get_json_paginated(&format!("{}/list", server.uri()), None)
          .await
          .unwrap();

        assert_eq!(result, vec![9]);
      }

      #[tokio::test]
      async fn it_fails_fast_when_a_later_page_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/list"))
          .and(query_param("page", "1"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Pages", "2")
              .set_body_raw(b"[1]".to_vec(), "application/json"),
          )
          .mount(&server)
          .await;
        Mock::given(method("GET"))
          .and(path("/list"))
          .and(query_param("page", "2"))
          .respond_with(ResponseTemplate::new(500))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result: Result<Vec<i32>, _> = client.get_json_paginated(&format!("{}/list", server.uri()), None).await;

        assert!(matches!(result, Err(Error::Http(_))));
      }

      #[tokio::test]
      async fn it_returns_rate_limit_error_when_the_first_page_is_429() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/list"))
          .and(query_param("page", "1"))
          .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "12"))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result: Result<Vec<i32>, _> = client.get_json_paginated(&format!("{}/list", server.uri()), None).await;

        assert!(matches!(
          result,
          Err(Error::RateLimit {
            retry_after_secs: 12
          })
        ));
      }
    }

    mod post_empty {
      use super::*;

      #[tokio::test]
      async fn it_returns_ok_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/things"))
          .respond_with(ResponseTemplate::new(204))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result = client
          .post_empty(&format!("{}/things", server.uri()), &serde_json::Value::Null, "token")
          .await;

        assert!(result.is_ok());
      }

      #[tokio::test]
      async fn it_returns_rate_limit_error_on_429() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/things"))
          .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "45"))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result = client
          .post_empty(&format!("{}/things", server.uri()), &serde_json::Value::Null, "token")
          .await;

        assert!(matches!(
          result,
          Err(Error::RateLimit {
            retry_after_secs: 45
          })
        ));
      }
    }

    mod post_form {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_returns_deserialized_response_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/token"))
          .respond_with(
            ResponseTemplate::new(200).set_body_raw(b"{\"access_token\":\"abc\"}".to_vec(), "application/json"),
          )
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;
        let body = [("grant_type", "authorization_code")];

        let result: serde_json::Value = client
          .post_form(&format!("{}/token", server.uri()), &body)
          .await
          .unwrap();

        assert_eq!(result["access_token"], "abc");
      }

      #[tokio::test]
      async fn it_returns_http_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/token"))
          .respond_with(ResponseTemplate::new(400))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;
        let body = [("grant_type", "authorization_code")];

        let result: Result<serde_json::Value, _> = client.post_form(&format!("{}/token", server.uri()), &body).await;

        assert!(matches!(result, Err(Error::Http(_))));
      }

      #[tokio::test]
      async fn it_returns_rate_limit_error_on_429() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/token"))
          .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "10"))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;
        let body = [("grant_type", "authorization_code")];

        let result: Result<serde_json::Value, _> = client.post_form(&format!("{}/token", server.uri()), &body).await;

        assert!(matches!(
          result,
          Err(Error::RateLimit {
            retry_after_secs: 10
          })
        ));
      }
    }

    mod post_json {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_returns_deserialized_response_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/items"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(b"{\"id\":42}".to_vec(), "application/json"))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result: serde_json::Value = client
          .post_json(
            &format!("{}/items", server.uri()),
            &serde_json::json!({"value": 1}),
            "token",
          )
          .await
          .unwrap();

        assert_eq!(result["id"], 42);
      }

      #[tokio::test]
      async fn it_returns_http_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/items"))
          .respond_with(ResponseTemplate::new(422))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result: Result<serde_json::Value, _> = client
          .post_json(
            &format!("{}/items", server.uri()),
            &serde_json::json!({"value": 1}),
            "token",
          )
          .await;

        assert!(matches!(result, Err(Error::Http(_))));
      }

      #[tokio::test]
      async fn it_returns_rate_limit_error_on_429() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/items"))
          .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "20"))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result: Result<serde_json::Value, _> = client
          .post_json(
            &format!("{}/items", server.uri()),
            &serde_json::json!({"value": 1}),
            "token",
          )
          .await;

        assert!(matches!(
          result,
          Err(Error::RateLimit {
            retry_after_secs: 20
          })
        ));
      }
    }

    mod post_json_anon {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_returns_deserialized_response_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/anon"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(b"{\"result\":\"ok\"}".to_vec(), "application/json"))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result: serde_json::Value = client
          .post_json_anon(&format!("{}/anon", server.uri()), &serde_json::json!({"key": "val"}))
          .await
          .unwrap();

        assert_eq!(result["result"], "ok");
      }

      #[tokio::test]
      async fn it_returns_rate_limit_error_on_429() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
          .and(path("/anon"))
          .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "15"))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result: Result<serde_json::Value, _> = client
          .post_json_anon(&format!("{}/anon", server.uri()), &serde_json::json!({"key": "val"}))
          .await;

        assert!(matches!(
          result,
          Err(Error::RateLimit {
            retry_after_secs: 15
          })
        ));
      }
    }

    mod put_empty {
      use super::*;

      #[tokio::test]
      async fn it_returns_ok_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
          .and(path("/things/1"))
          .respond_with(ResponseTemplate::new(204))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result = client
          .put_empty(
            &format!("{}/things/1", server.uri()),
            &serde_json::json!({"read": true}),
            "token",
          )
          .await;

        assert!(result.is_ok());
      }

      #[tokio::test]
      async fn it_sends_the_bearer_token() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
          .and(path("/things/1"))
          .and(header("Authorization", "Bearer secret-token"))
          .respond_with(ResponseTemplate::new(204))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result = client
          .put_empty(
            &format!("{}/things/1", server.uri()),
            &serde_json::json!({"read": true}),
            "secret-token",
          )
          .await;

        assert!(result.is_ok());
      }

      #[tokio::test]
      async fn it_returns_http_error_on_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
          .and(path("/things/1"))
          .respond_with(ResponseTemplate::new(403))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result = client
          .put_empty(
            &format!("{}/things/1", server.uri()),
            &serde_json::json!({"read": true}),
            "token",
          )
          .await;

        assert!(matches!(result, Err(Error::Http(_))));
      }

      #[tokio::test]
      async fn it_returns_rate_limit_error_on_429() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
          .and(path("/things/1"))
          .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "25"))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result = client
          .put_empty(
            &format!("{}/things/1", server.uri()),
            &serde_json::json!({"read": true}),
            "token",
          )
          .await;

        assert!(matches!(
          result,
          Err(Error::RateLimit {
            retry_after_secs: 25
          })
        ));
      }
    }
  }

  mod rate_limiter {
    use super::*;

    mod check {
      use std::time::{Duration, Instant};

      use super::*;

      #[tokio::test]
      async fn it_returns_immediately_when_below_capacity() {
        let mut limiter = RateLimiter::new(3, Duration::from_secs(60));

        let start = Instant::now();
        limiter.check().await;
        limiter.check().await;

        assert!(start.elapsed() < Duration::from_millis(50));
      }

      #[tokio::test]
      async fn it_sleeps_until_capacity_is_available() {
        let window = Duration::from_millis(100);
        let mut limiter = RateLimiter::new(2, window);

        limiter.check().await;
        limiter.check().await;

        let start = Instant::now();
        limiter.check().await;

        assert!(start.elapsed() >= window);
      }
    }
  }
}
