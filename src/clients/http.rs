use std::{
  collections::HashMap,
  sync::{Arc, Mutex},
  time::{Duration, Instant},
};

use chrono::Utc;
use futures_util::{StreamExt, stream};
use reqwest::header::HeaderMap;
use serde::{Serialize, de::DeserializeOwned};

use crate::{
  clients::Error,
  store::{self, model::HttpCacheEntry, repo::infra},
};

const COMPATIBILITY_DATE_HEADER: &str = "X-Compatibility-Date";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const ESI_ERROR_LIMIT_RESET_HEADER: &str = "X-ESI-Error-Limit-Reset";
const ESI_PAGES_HEADER: &str = "X-Pages";
const HTTP_TARGET: &str = "pod::http";
// Cap on simultaneous in-flight requests for the page-2..N fan-out of a paginated endpoint. The old
// unbounded JoinSet opened one connection per page at once, so a single large list (assets, journal)
// could spike dozens of concurrent requests — and, via their cache upserts, dozens of writes — at
// the single SQLite writer. Bounding the fan-out caps that burst while still overlapping enough page
// fetches to stay fast.
const MAX_CONCURRENT_PAGES: usize = 6;
// Soft size threshold for the write-behind HTTP cache buffer. When buffered (unflushed) cache
// entries reach this many, an upsert eagerly flushes the batch so a long-running job can't grow the
// buffer without bound. A per-job flush in the sync engine bounds it on the time axis.
const CACHE_FLUSH_THRESHOLD: usize = 32;
const RATELIMIT_GROUP_HEADER: &str = "X-Ratelimit-Group";
const RATELIMIT_LIMIT_HEADER: &str = "X-Ratelimit-Limit";
const RATELIMIT_REMAINING_HEADER: &str = "X-Ratelimit-Remaining";
const RATELIMIT_USED_HEADER: &str = "X-Ratelimit-Used";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// HTTP response cache backed by the `http_cache` table, fronted by a write-behind buffer.
///
/// Every ESI call reads the cache before the request and writes it after, so a multi-character sync
/// used to issue hundreds of one-row write transactions against the single SQLite writer. Instead,
/// `upsert` records entries in an in-memory buffer keyed by URL and they are flushed to the database
/// in a single batched transaction — on a size threshold, or explicitly per sync job. Reads consult
/// the buffer before the database, so a job always sees its own un-flushed writes (read-your-writes),
/// and cache hit/miss/freshness behavior is identical to writing through every time.
pub struct Cache {
  db: store::Database,
  pending: Mutex<HashMap<String, HttpCacheEntry>>,
}

impl Cache {
  pub fn new(db: store::Database) -> Self {
    Self {
      db,
      pending: Mutex::new(HashMap::new()),
    }
  }

  // Reached only through Client::cache_db (a test-support seam), so dead in the lib build but live
  // under tests; #[expect] would be unfulfilled there.
  #[allow(dead_code)]
  pub(crate) fn db(&self) -> &store::Database {
    &self.db
  }

  async fn get(&self, url: &str) -> Result<Option<HttpCacheEntry>, store::Error> {
    // Read-your-writes: a buffered (not-yet-flushed) entry shadows the database row for the same URL.
    if let Some(entry) = self.buffered(url) {
      return Ok(Some(entry));
    }
    infra::http_cache_get(&self.db, url).await
  }

  fn buffered(&self, url: &str) -> Option<HttpCacheEntry> {
    self
      .pending
      .lock()
      .expect("http cache buffer mutex poisoned")
      .get(url)
      .cloned()
  }

  async fn upsert(&self, entry: &HttpCacheEntry) -> Result<(), store::Error> {
    let pending = {
      let mut buffer = self.pending.lock().expect("http cache buffer mutex poisoned");
      buffer.insert(entry.url().clone(), entry.clone());
      if buffer.len() < CACHE_FLUSH_THRESHOLD {
        return Ok(());
      }
      std::mem::take(&mut *buffer)
    };
    write_batch(&self.db, pending).await
  }

  /// Flush every buffered entry to the database in a single transaction. Anchored per sync job so a
  /// job's cache writes land as one batched write rather than one transaction per ESI call.
  async fn flush(&self) -> Result<(), store::Error> {
    let pending = {
      let mut buffer = self.pending.lock().expect("http cache buffer mutex poisoned");
      if buffer.is_empty() {
        return Ok(());
      }
      std::mem::take(&mut *buffer)
    };
    write_batch(&self.db, pending).await
  }
}

async fn write_batch(db: &store::Database, entries: HashMap<String, HttpCacheEntry>) -> Result<(), store::Error> {
  // A lone entry is already a single atomic statement, so skip the explicit transaction and reuse the
  // shared single-row upsert; only a real batch is worth wrapping in one BEGIN/COMMIT.
  if entries.len() == 1 {
    if let Some(entry) = entries.values().next() {
      return infra::http_cache_upsert(db, entry).await;
    }
    return Ok(());
  }
  let mut tx = db.writer().begin().await?;
  for entry in entries.values() {
    sqlx::query("INSERT OR REPLACE INTO http_cache (body, cached_at, etag, expires_at, url) VALUES (?, ?, ?, ?, ?)")
      .bind(entry.body())
      .bind(entry.cached_at())
      .bind(entry.etag().as_deref())
      .bind(entry.expires_at())
      .bind(entry.url().as_str())
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;
  Ok(())
}

pub struct Client {
  budgets: Arc<RateBudgets>,
  cache: Cache,
  inner: reqwest::Client,
}

impl Client {
  pub fn builder(cache: Cache) -> ClientBuilder {
    ClientBuilder {
      cache,
    }
  }

  pub async fn delete_empty(&self, url: &str, token: &str, compat_date: Option<&str>) -> Result<(), Error> {
    let mut req = self.inner.delete(url).bearer_auth(token);
    if let Some(date) = compat_date {
      req = req.header(COMPATIBILITY_DATE_HEADER, date);
    }
    let resp = send_logged("DELETE", url, req, &self.budgets).await?;
    handle_status(resp).await
  }

  pub async fn get_bytes_uncached(&self, url: &str) -> Result<Vec<u8>, Error> {
    self.get_bytes_uncached_inner(url, None).await
  }

  pub async fn get_bytes_uncached_with_timeout(&self, url: &str, timeout: Duration) -> Result<Vec<u8>, Error> {
    self.get_bytes_uncached_inner(url, Some(timeout)).await
  }

  pub async fn get_json<T: DeserializeOwned>(
    &self,
    url: &str,
    token: Option<&str>,
    compat_date: Option<&str>,
  ) -> Result<T, Error> {
    let body = self.get_cached_bytes(url, token, false, compat_date).await?;
    Ok(serde_json::from_slice(&body)?)
  }

  pub async fn get_json_paginated<T: DeserializeOwned + Send + 'static>(
    &self,
    url: &str,
    token: Option<&str>,
    compat_date: Option<&'static str>,
  ) -> Result<Vec<T>, Error> {
    let (mut items, total_pages) = fetch_page::<T>(&self.inner, &self.budgets, url, 1, token, compat_date).await?;

    if total_pages <= 1 {
      return Ok(items);
    }

    // Fetch the remaining pages with a bounded number of simultaneous requests instead of an
    // unbounded fan-out, capping the concurrent connection/write burst at the single SQLite writer.
    let mut pages = stream::iter(2..=total_pages)
      .map(|page| {
        let inner = &self.inner;
        let budgets = &self.budgets;
        async move {
          fetch_page::<T>(inner, budgets, url, page, token, compat_date)
            .await
            .map(|(items, _)| items)
        }
      })
      .buffer_unordered(MAX_CONCURRENT_PAGES);

    while let Some(page_items) = pages.next().await {
      items.extend(page_items?);
    }

    Ok(items)
  }

  pub async fn post_form<B: Serialize, T: DeserializeOwned>(&self, url: &str, body: &B) -> Result<T, Error> {
    let resp = send_logged("POST", url, self.inner.post(url).form(body), &self.budgets).await?;
    deserialize_response(resp).await
  }

  pub async fn post_json<B: Serialize, T: DeserializeOwned>(
    &self,
    url: &str,
    body: &B,
    token: &str,
    compat_date: Option<&str>,
  ) -> Result<T, Error> {
    let mut req = self.inner.post(url).bearer_auth(token).json(body);
    if let Some(date) = compat_date {
      req = req.header(COMPATIBILITY_DATE_HEADER, date);
    }
    let resp = send_logged("POST", url, req, &self.budgets).await?;
    deserialize_response(resp).await
  }

  pub async fn post_json_anon<B: Serialize, T: DeserializeOwned>(
    &self,
    url: &str,
    body: &B,
    compat_date: Option<&str>,
  ) -> Result<T, Error> {
    let mut req = self.inner.post(url).json(body);
    if let Some(date) = compat_date {
      req = req.header(COMPATIBILITY_DATE_HEADER, date);
    }
    let resp = send_logged("POST", url, req, &self.budgets).await?;
    deserialize_response(resp).await
  }

  pub async fn put_empty<B: Serialize>(
    &self,
    url: &str,
    body: &B,
    token: &str,
    compat_date: Option<&str>,
  ) -> Result<(), Error> {
    let mut req = self.inner.put(url).bearer_auth(token).json(body);
    if let Some(date) = compat_date {
      req = req.header(COMPATIBILITY_DATE_HEADER, date);
    }
    let resp = send_logged("PUT", url, req, &self.budgets).await?;
    handle_status(resp).await
  }

  /// Flush the write-behind HTTP cache buffer to the database. The sync engine calls this once after
  /// each job so a job's many ESI cache writes are coalesced into a single batched transaction
  /// instead of one transaction per request, cutting the writer's queue depth during a sync.
  pub async fn flush_cache(&self) -> Result<(), store::Error> {
    self.cache.flush().await
  }

  // Test-support accessor: the app-level cache-population test reads the cache database through this
  // seam. Dead in the lib build but used by tests, so #[expect] would be unfulfilled there.
  #[allow(dead_code)]
  pub(crate) fn cache_db(&self) -> &store::Database {
    self.cache.db()
  }

  async fn get_bytes_uncached_inner(&self, url: &str, timeout: Option<Duration>) -> Result<Vec<u8>, Error> {
    let mut req = self.inner.get(url);
    if let Some(timeout) = timeout {
      req = req.timeout(timeout);
    }
    let resp = send_logged("GET", url, req, &self.budgets).await?;
    if let Some(err) = throttle_error(&resp) {
      return Err(err);
    }
    if !(200..300).contains(&resp.status().as_u16()) {
      return Err(Error::Http(resp.error_for_status().unwrap_err()));
    }
    Ok(resp.bytes().await?.to_vec())
  }

  async fn get_cached_bytes(
    &self,
    url: &str,
    token: Option<&str>,
    serve_fresh: bool,
    compat_date: Option<&str>,
  ) -> Result<Vec<u8>, Error> {
    let cached = self.cache.get(url).await?;

    if serve_fresh
      && let Some(ref entry) = cached
      && !entry.is_expired()
    {
      tracing::trace!(target: HTTP_TARGET, method = "GET", url, cache = "hit", "served from fresh cache");
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
    if let Some(date) = compat_date {
      req = req.header(COMPATIBILITY_DATE_HEADER, date);
    }

    let resp = send_logged("GET", url, req, &self.budgets).await?;
    let status = resp.status().as_u16();

    if status == 304 {
      tracing::trace!(target: HTTP_TARGET, method = "GET", url, status, cache = "not-modified", "revalidated; served from cache");
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
      budgets: Arc::new(RateBudgets::new()),
      cache: self.cache,
      inner,
    })
  }
}

#[derive(Default)]
struct BudgetState {
  groups: HashMap<String, GroupBudget>,
  routes: HashMap<String, String>,
}

struct GroupBudget {
  limit: u32,
  next_allowed_at: Instant,
  remaining: u32,
  reset_at: Instant,
  window: Duration,
}

struct ParsedBudget {
  group: String,
  limit: u32,
  remaining: u32,
  window: Duration,
}

struct RateBudgets {
  state: Mutex<BudgetState>,
}

impl RateBudgets {
  fn new() -> Self {
    Self {
      state: Mutex::new(BudgetState::default()),
    }
  }

  fn record(&self, route_key: &str, parsed: ParsedBudget, now: Instant) {
    let reset_at = now + parsed.window;
    let mut state = self.state.lock().expect("rate-budget mutex poisoned");

    state.routes.insert(route_key.to_owned(), parsed.group.clone());

    match state.groups.get_mut(&parsed.group) {
      Some(budget) => {
        budget.limit = parsed.limit;
        budget.remaining = parsed.remaining;
        budget.reset_at = reset_at;
        budget.window = parsed.window;
        if budget.next_allowed_at < now {
          budget.next_allowed_at = now;
        }
      }
      None => {
        state.groups.insert(
          parsed.group,
          GroupBudget {
            limit: parsed.limit,
            next_allowed_at: now,
            remaining: parsed.remaining,
            reset_at,
            window: parsed.window,
          },
        );
      }
    }
  }

  fn reserve_slot(&self, route_key: &str, now: Instant) -> Option<Duration> {
    let mut state = self.state.lock().expect("rate-budget mutex poisoned");

    let group = state.routes.get(route_key)?.clone();
    let budget = state.groups.get_mut(&group)?;

    if now >= budget.reset_at {
      budget.remaining = budget.limit;
      budget.next_allowed_at = now;
      return None;
    }

    // Gate only when remaining falls to ≤ 20% of limit; leave headroom for concurrent requests.
    let low_watermark = (budget.limit / 5).max(1);
    if budget.remaining > low_watermark {
      return None;
    }

    let (earliest, spacing) = if budget.remaining == 0 {
      (budget.reset_at, budget.window / budget.limit.max(1))
    } else {
      let remaining_window = budget.reset_at.saturating_duration_since(now);
      (now, remaining_window / budget.remaining)
    };
    let slot = budget.next_allowed_at.max(earliest);
    budget.next_allowed_at = slot + spacing;

    Some(slot.saturating_duration_since(now))
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
  budgets: &RateBudgets,
  url_base: &str,
  page: u32,
  token: Option<&str>,
  compat_date: Option<&str>,
) -> Result<(Vec<T>, u32), Error> {
  let separator = if url_base.contains('?') { '&' } else { '?' };
  let url = format!("{url_base}{separator}page={page}");
  let mut req = inner.get(&url);
  if let Some(t) = token {
    req = req.bearer_auth(t);
  }
  if let Some(date) = compat_date {
    req = req.header(COMPATIBILITY_DATE_HEADER, date);
  }

  let resp = send_logged("GET", &url, req, budgets).await?;
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

fn is_id_segment(seg: &str) -> bool {
  !seg.is_empty()
    // len >= 8 collapses killmail SHA-hash segments (≥ 40 hex chars) without swallowing short words like "ok"
    && (seg.bytes().all(|b| b.is_ascii_digit()) || (seg.len() >= 8 && seg.bytes().all(|b| b.is_ascii_hexdigit())))
}

fn parse_error_limit_reset(resp: &reqwest::Response) -> u64 {
  resp
    .headers()
    .get(ESI_ERROR_LIMIT_RESET_HEADER)
    .and_then(|v| v.to_str().ok())
    .and_then(|s| s.parse::<u64>().ok())
    .unwrap_or(60)
}

fn parse_rate_headers(headers: &HeaderMap) -> Option<ParsedBudget> {
  let group = headers.get(RATELIMIT_GROUP_HEADER)?.to_str().ok()?.to_owned();
  let (limit, window) = parse_rate_limit_spec(headers.get(RATELIMIT_LIMIT_HEADER)?.to_str().ok()?)?;
  let remaining = headers
    .get(RATELIMIT_REMAINING_HEADER)
    .and_then(|v| v.to_str().ok())
    .and_then(|s| s.parse::<u32>().ok())
    .or_else(|| {
      headers
        .get(RATELIMIT_USED_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok())
        .map(|used| limit.saturating_sub(used))
    })
    .unwrap_or(limit);

  Some(ParsedBudget {
    group,
    limit,
    remaining,
    window,
  })
}

fn parse_rate_limit_spec(spec: &str) -> Option<(u32, Duration)> {
  let (count, window) = spec.split_once('/')?;
  let limit = count.trim().parse::<u32>().ok()?;
  let window = window.trim();
  let split = window.find(|c: char| !c.is_ascii_digit())?;
  let (value, unit) = window.split_at(split);
  let value = value.parse::<u64>().ok()?;
  let secs = match unit {
    "s" => value,
    "m" => value * 60,
    "h" => value * 3600,
    _ => return None,
  };

  Some((limit, Duration::from_secs(secs)))
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

fn route_key(path: &str) -> String {
  path
    .split('/')
    .map(|seg| if is_id_segment(seg) { "{}" } else { seg })
    .collect::<Vec<_>>()
    .join("/")
}

async fn send_logged(
  method: &str,
  url: &str,
  req: reqwest::RequestBuilder,
  budgets: &RateBudgets,
) -> Result<reqwest::Response, Error> {
  let key = route_key(url_path(url));
  if let Some(delay) = budgets.reserve_slot(&key, Instant::now())
    && !delay.is_zero()
  {
    tracing::trace!(target: HTTP_TARGET, method, url, delay_ms = delay.as_millis() as u64, "spacing request to respect rate-limit budget");
    tokio::time::sleep(delay).await;
  }

  let started = Instant::now();
  let result = req.send().await;
  let elapsed_ms = started.elapsed().as_millis() as u64;

  match result {
    Ok(resp) => {
      if let Some(parsed) = parse_rate_headers(resp.headers()) {
        budgets.record(&key, parsed, Instant::now());
      }
      let status = resp.status().as_u16();
      if (200..400).contains(&status) {
        tracing::trace!(target: HTTP_TARGET, method, url, status, elapsed_ms, "request completed");
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
          .delete_empty(&format!("{}/things/1", server.uri()), "token", None)
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
          .delete_empty(&format!("{}/things/1", server.uri()), "token", None)
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
          .delete_empty(&format!("{}/things/1", server.uri()), "token", None)
          .await;

        assert!(matches!(
          result,
          Err(Error::RateLimit {
            retry_after_secs: 30
          })
        ));
      }
    }

    mod get_json {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_learns_a_group_budget_from_response_headers() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/killmails/123/abcdef0123456789/"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("X-Ratelimit-Group", "killmails")
              .insert_header("X-Ratelimit-Limit", "100/15m")
              .insert_header("X-Ratelimit-Remaining", "1")
              .set_body_raw(b"[]".to_vec(), "application/json"),
          )
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;
        let url = format!("{}/killmails/123/abcdef0123456789/", server.uri());

        let _: Vec<i32> = client.get_json(&url, None, None).await.unwrap();

        let key = route_key(url_path(&url));
        assert!(client.budgets.reserve_slot(&key, std::time::Instant::now()).is_some());
      }

      #[tokio::test]
      async fn it_omits_the_compatibility_date_header_when_not_provided() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/resource"))
          .and(|req: &wiremock::Request| !req.headers.contains_key("X-Compatibility-Date"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(b"[1]".to_vec(), "application/json"))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result: Vec<i32> = client
          .get_json(&format!("{}/resource", server.uri()), None, None)
          .await
          .unwrap();

        assert_eq!(result, vec![1]);
      }

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

        let result: Vec<i32> = client.get_json(&url, None, None).await.unwrap();

        assert_eq!(result, vec![7, 8, 9]);
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

        let result: Result<Vec<i32>, _> = client.get_json(&format!("{}/resource", server.uri()), None, None).await;

        assert!(matches!(
          result,
          Err(Error::ErrorLimited {
            reset_secs: 8
          })
        ));
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

        let result: Result<Vec<i32>, _> = client.get_json(&format!("{}/resource", server.uri()), None, None).await;

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

        let result: Result<Vec<i32>, _> = client.get_json(&format!("{}/resource", server.uri()), None, None).await;

        assert!(matches!(
          result,
          Err(Error::RateLimit {
            retry_after_secs: 60
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

        let result: Vec<i32> = client.get_json(&url, None, None).await.unwrap();

        assert_eq!(result, vec![1]);
      }

      #[tokio::test]
      async fn it_sends_the_compatibility_date_header_when_provided() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/resource"))
          .and(header("X-Compatibility-Date", "2026-06-08"))
          .respond_with(ResponseTemplate::new(200).set_body_raw(b"[1]".to_vec(), "application/json"))
          .mount(&server)
          .await;
        let (client, _db) = make_test_client().await;

        let result: Vec<i32> = client
          .get_json(&format!("{}/resource", server.uri()), None, Some("2026-06-08"))
          .await
          .unwrap();

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

        let _: Vec<i32> = client.get_json(&url, None, None).await.unwrap();
        client.flush_cache().await.unwrap();

        let cached = infra::http_cache_get(&db, &url).await.unwrap().unwrap();
        assert_eq!(cached.etag().as_deref(), Some("\"new-etag\""));
        assert_eq!(cached.body(), b"[1,2,3]");
      }
    }

    mod get_json_paginated {
      use pretty_assertions::assert_eq;

      use super::*;

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

        let result: Result<Vec<i32>, _> = client
          .get_json_paginated(&format!("{}/list", server.uri()), None, None)
          .await;

        assert!(matches!(result, Err(Error::Http(_))));
      }

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
          .get_json_paginated(&format!("{}/list", server.uri()), None, None)
          .await
          .unwrap();
        result.sort();

        assert_eq!(result, vec![1, 2, 3, 4, 5, 6]);
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

        let result: Result<Vec<i32>, _> = client
          .get_json_paginated(&format!("{}/list", server.uri()), None, None)
          .await;

        assert!(matches!(
          result,
          Err(Error::RateLimit {
            retry_after_secs: 12
          })
        ));
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
          .get_json_paginated(&format!("{}/list", server.uri()), None, None)
          .await
          .unwrap();

        assert_eq!(result, vec![9]);
      }
    }

    mod cache_coalescing {
      use pretty_assertions::assert_eq;

      use super::*;

      #[tokio::test]
      async fn it_serves_a_buffered_upsert_before_it_is_flushed() {
        let db = store::open_test().await.unwrap();
        let cache = Cache::new(db.clone());
        let entry = HttpCacheEntry::new(b"[1,2,3]".to_vec(), 0, "https://esi.example/buffered");

        cache.upsert(&entry).await.unwrap();

        let got = cache.get("https://esi.example/buffered").await.unwrap();
        assert_eq!(got.as_ref().map(HttpCacheEntry::body), Some(&b"[1,2,3]".to_vec()));
        assert!(
          infra::http_cache_get(&db, "https://esi.example/buffered")
            .await
            .unwrap()
            .is_none(),
          "a single upsert stays buffered in memory and is not yet written through to the database"
        );
      }

      #[tokio::test]
      async fn it_flushes_buffered_entries_to_the_database_in_one_pass() {
        let db = store::open_test().await.unwrap();
        let cache = Cache::new(db.clone());
        cache
          .upsert(&HttpCacheEntry::new(b"a".to_vec(), 0, "https://esi.example/a"))
          .await
          .unwrap();
        cache
          .upsert(&HttpCacheEntry::new(b"b".to_vec(), 0, "https://esi.example/b"))
          .await
          .unwrap();

        cache.flush().await.unwrap();

        assert!(
          infra::http_cache_get(&db, "https://esi.example/a")
            .await
            .unwrap()
            .is_some()
        );
        assert!(
          infra::http_cache_get(&db, "https://esi.example/b")
            .await
            .unwrap()
            .is_some()
        );
      }

      #[tokio::test]
      async fn it_serves_the_latest_buffered_upsert_for_a_url() {
        let db = store::open_test().await.unwrap();
        let cache = Cache::new(db);
        cache
          .upsert(&HttpCacheEntry::new(b"old".to_vec(), 0, "https://esi.example/dup"))
          .await
          .unwrap();
        cache
          .upsert(&HttpCacheEntry::new(b"new".to_vec(), 1, "https://esi.example/dup"))
          .await
          .unwrap();

        let got = cache.get("https://esi.example/dup").await.unwrap().unwrap();

        assert_eq!(
          got.body(),
          b"new",
          "a re-upsert of the same url replaces the buffered entry"
        );
      }

      #[tokio::test]
      async fn it_auto_flushes_once_the_buffer_reaches_the_threshold() {
        let db = store::open_test().await.unwrap();
        let cache = Cache::new(db.clone());
        for i in 0..CACHE_FLUSH_THRESHOLD {
          cache
            .upsert(&HttpCacheEntry::new(
              vec![0],
              0,
              format!("https://esi.example/auto/{i}"),
            ))
            .await
            .unwrap();
        }

        assert!(
          infra::http_cache_get(&db, "https://esi.example/auto/0")
            .await
            .unwrap()
            .is_some(),
          "reaching the size threshold flushes the whole buffer to the database without an explicit flush"
        );
      }

      #[tokio::test]
      async fn it_persists_a_response_through_flush_cache_so_a_later_revalidation_sends_if_none_match() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
          .and(path("/resource"))
          .respond_with(
            ResponseTemplate::new(200)
              .insert_header("ETag", "\"flushed-etag\"")
              .set_body_raw(b"[7]".to_vec(), "application/json"),
          )
          .mount(&server)
          .await;
        let db = store::open_test().await.unwrap();
        let url = format!("{}/resource", server.uri());
        let client = Client::builder(Cache::new(db.clone())).build();

        let _: Vec<i32> = client.get_json(&url, None, None).await.unwrap();
        assert!(
          infra::http_cache_get(&db, &url).await.unwrap().is_none(),
          "the freshly fetched response is buffered, not yet written through"
        );

        client.flush_cache().await.unwrap();

        let cached = infra::http_cache_get(&db, &url).await.unwrap().unwrap();
        assert_eq!(cached.etag().as_deref(), Some("\"flushed-etag\""));
        assert_eq!(cached.body(), b"[7]");
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
            None,
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
            None,
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
            None,
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
          .post_json_anon(
            &format!("{}/anon", server.uri()),
            &serde_json::json!({"key": "val"}),
            None,
          )
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
          .post_json_anon(
            &format!("{}/anon", server.uri()),
            &serde_json::json!({"key": "val"}),
            None,
          )
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
            None,
          )
          .await;

        assert!(matches!(result, Err(Error::Http(_))));
      }

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
            None,
          )
          .await;

        assert!(result.is_ok());
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
            None,
          )
          .await;

        assert!(matches!(
          result,
          Err(Error::RateLimit {
            retry_after_secs: 25
          })
        ));
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
            None,
          )
          .await;

        assert!(result.is_ok());
      }
    }
  }

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

  mod parse_rate_headers {
    use pretty_assertions::assert_eq;

    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
      let mut map = HeaderMap::new();
      for (name, value) in pairs {
        map.insert(
          reqwest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
          value.parse().unwrap(),
        );
      }
      map
    }

    #[test]
    fn it_derives_remaining_from_used_when_remaining_is_absent() {
      let map = headers(&[
        ("X-Ratelimit-Group", "killmails"),
        ("X-Ratelimit-Limit", "150/15m"),
        ("X-Ratelimit-Used", "110"),
      ]);

      let parsed = parse_rate_headers(&map).unwrap();

      assert_eq!(parsed.remaining, 40);
    }

    #[test]
    fn it_parses_all_fields() {
      let map = headers(&[
        ("X-Ratelimit-Group", "killmails"),
        ("X-Ratelimit-Limit", "150/15m"),
        ("X-Ratelimit-Remaining", "42"),
        ("X-Ratelimit-Used", "108"),
      ]);

      let parsed = parse_rate_headers(&map).unwrap();

      assert_eq!(parsed.group, "killmails");
      assert_eq!(parsed.limit, 150);
      assert_eq!(parsed.remaining, 42);
      assert_eq!(parsed.window, Duration::from_secs(900));
    }

    #[test]
    fn it_returns_none_without_a_group() {
      let map = headers(&[("X-Ratelimit-Limit", "150/15m")]);

      assert!(parse_rate_headers(&map).is_none());
    }
  }

  mod parse_rate_limit_spec {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_hours() {
      let (limit, window) = parse_rate_limit_spec("60/1h").unwrap();

      assert_eq!(limit, 60);
      assert_eq!(window, Duration::from_secs(3600));
    }

    #[test]
    fn it_parses_minutes() {
      let (limit, window) = parse_rate_limit_spec("150/15m").unwrap();

      assert_eq!(limit, 150);
      assert_eq!(window, Duration::from_secs(900));
    }

    #[test]
    fn it_returns_none_for_an_unknown_unit() {
      assert!(parse_rate_limit_spec("150/15d").is_none());
    }

    #[test]
    fn it_returns_none_without_a_window() {
      assert!(parse_rate_limit_spec("150").is_none());
    }
  }

  mod rate_budgets {
    use std::time::{Duration, Instant};

    use super::*;

    fn parsed(group: &str, limit: u32, remaining: u32, window: Duration) -> ParsedBudget {
      ParsedBudget {
        group: group.to_owned(),
        limit,
        remaining,
        window,
      }
    }

    #[test]
    fn it_clears_gating_after_the_window_elapses() {
      let budgets = RateBudgets::new();
      let now = Instant::now();
      budgets.record("/k/", parsed("k", 100, 0, Duration::from_secs(60)), now);

      let after_reset = now + Duration::from_secs(61);

      assert!(budgets.reserve_slot("/k/", after_reset).is_none());
    }

    #[test]
    fn it_does_not_gate_an_unknown_route() {
      let budgets = RateBudgets::new();

      assert!(budgets.reserve_slot("/unknown/", Instant::now()).is_none());
    }

    #[test]
    fn it_does_not_gate_when_remaining_is_high() {
      let budgets = RateBudgets::new();
      let now = Instant::now();
      budgets.record("/k/", parsed("k", 100, 90, Duration::from_secs(60)), now);

      assert!(budgets.reserve_slot("/k/", now).is_none());
    }

    #[test]
    fn it_spaces_requests_when_remaining_is_low() {
      let budgets = RateBudgets::new();
      let now = Instant::now();
      budgets.record("/k/", parsed("k", 100, 5, Duration::from_secs(60)), now);

      let first = budgets.reserve_slot("/k/", now).unwrap();
      let second = budgets.reserve_slot("/k/", now).unwrap();

      assert_eq!(first, Duration::ZERO);
      assert!(second > Duration::ZERO);
    }

    #[test]
    fn it_waits_for_the_window_to_reset_when_remaining_is_zero() {
      let budgets = RateBudgets::new();
      let now = Instant::now();
      budgets.record("/k/", parsed("k", 100, 0, Duration::from_secs(60)), now);

      let delay = budgets.reserve_slot("/k/", now).unwrap();

      assert!(delay >= Duration::from_secs(59));
      assert!(delay <= Duration::from_secs(60));
    }
  }

  mod route_key {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_leaves_short_words_intact() {
      assert_eq!(route_key("/status/"), "/status/");
    }

    #[test]
    fn it_replaces_killmail_id_and_hash_segments() {
      assert_eq!(
        route_key("/killmails/123456/a1b2c3d4e5f6a7b8c9d0/"),
        "/killmails/{}/{}/"
      );
    }

    #[test]
    fn it_replaces_numeric_id_segments() {
      assert_eq!(route_key("/characters/12345/assets/"), "/characters/{}/assets/");
    }
  }
}
