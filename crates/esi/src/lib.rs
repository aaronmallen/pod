//! ESI (EVE Swagger Interface) client and models.

mod cache;
mod clients;
mod http;
pub mod models;
#[cfg(feature = "pod-model")]
mod pod_model;
pub mod scopes;

use std::sync::Arc;

pub use cache::CacheType;
use cache::Store as CacheStore;
use models::auth::Grant;
use validator::Validate;

/// Errors that can occur when interacting with the ESI.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// A non-2xx API response with its HTTP status and error message.
  #[error("api error {status}: {error}")]
  Api {
    /// The HTTP status code.
    status: u16,
    /// The error message from the API response body.
    error: String,
  },
  /// An OAuth2 authentication error.
  #[error("authentication error: {0}")]
  Authentication(String),
  /// An HTTP transport or protocol error.
  #[error("http error: {0}")]
  Http(#[from] reqwest::Error),
  /// An internal logic error.
  #[error("internal error: {0}")]
  Internal(String),
  /// A JSON parse error.
  #[error("parse error: {0}")]
  Parse(#[from] serde_json::Error),
  /// The ESI rate limit was hit; retry after the given number of seconds.
  #[error("rate limited; retry after {retry_after_secs}s")]
  RateLimit {
    /// Seconds to wait before retrying.
    retry_after_secs: u64,
  },
}

/// Top-level ESI client.
#[derive(Clone, Debug)]
pub struct Client {
  pub(crate) base_url: String,
  client_id: String,
  http: http::Client,
}

impl Client {
  /// Returns a new [`ClientBuilder`] pre-configured with the given EVE SSO client ID.
  pub fn builder(client_id: impl Into<String>) -> ClientBuilder {
    ClientBuilder::new(client_id)
  }

  /// Returns the EVE SSO client ID.
  pub fn id(&self) -> &str {
    &self.client_id
  }

  /// Returns a reference to the internal HTTP client.
  pub(crate) fn http(&self) -> &http::Client {
    &self.http
  }

  /// Returns a `UrlBuilder` pre-configured with this client's base URL.
  pub(crate) fn url_builder(&self) -> http::UrlBuilder {
    http::UrlBuilder::new(&self.base_url)
  }

  /// Returns an auth client for performing SSO operations.
  pub fn auth(&self) -> clients::auth::Client<'_> {
    clients::auth::Client::new(self)
  }

  /// Returns a client for the given alliance.
  pub fn alliance(&self, id: i64) -> clients::alliance::Client<'_> {
    clients::alliance::Client::new(self, id)
  }

  /// Returns an authenticated character client bound to the given grant.
  pub fn character<'a>(&'a self, grant: &'a Grant) -> clients::character::AuthenticatedClient<'a> {
    clients::character::AuthenticatedClient::new(self, grant)
  }

  /// Returns a public (unauthenticated) client for the character with the given ID.
  pub fn character_public(&self, id: i64) -> clients::character::Client<'_> {
    clients::character::Client::new(self, id)
  }

  /// Returns a client for the given corporation.
  pub fn corporation(&self, id: i64) -> clients::corporation::Client<'_> {
    clients::corporation::Client::new(self, id)
  }

  /// Returns a client for public contract endpoints.
  pub fn contract(&self) -> clients::contract::Client<'_> {
    clients::contract::Client::new(self)
  }

  /// Returns a client for dogma endpoints.
  pub fn dogma(&self) -> clients::dogma::Client<'_> {
    clients::dogma::Client::new(self)
  }

  /// Returns a client for faction warfare endpoints.
  pub fn faction_warfare(&self) -> clients::faction_warfare::Client<'_> {
    clients::faction_warfare::Client::new(self)
  }

  /// Returns a client for the given fleet.
  pub fn fleet(&self, id: i64) -> clients::fleet::Client<'_> {
    clients::fleet::Client::new(self, id)
  }

  /// Returns a client for fetching images from the EVE image server.
  pub fn images(&self) -> clients::images::Client<'_> {
    clients::images::Client::new(self)
  }

  /// Returns a client for industry endpoints.
  pub fn industry(&self) -> clients::industry::Client<'_> {
    clients::industry::Client::new(self)
  }

  /// Returns a client for insurance endpoints.
  pub fn insurance(&self) -> clients::insurance::Client<'_> {
    clients::insurance::Client::new(self)
  }

  /// Returns a client for the given killmail.
  pub fn killmail(&self, id: i64, hash: &str) -> clients::killmail::Client<'_> {
    clients::killmail::Client::new(self, id, hash)
  }

  /// Returns a client for a specific NPC corporation loyalty store.
  pub fn loyalty(&self, corp_id: i64) -> clients::loyalty::Client<'_> {
    clients::loyalty::Client::new(self, corp_id)
  }

  /// Returns a client for market endpoints.
  pub fn market(&self) -> clients::market::Client<'_> {
    clients::market::Client::new(self)
  }

  /// Returns a client for market order endpoints (unauthenticated, Jita price lookup).
  pub fn markets(&self) -> clients::markets::Client<'_> {
    clients::markets::Client::new(self)
  }

  /// Returns a client for sovereignty endpoints.
  pub fn sovereignty(&self) -> clients::sovereignty::Client<'_> {
    clients::sovereignty::Client::new(self)
  }

  /// Returns a client for downloading EVE static data exports.
  pub fn static_data(&self) -> clients::static_data::Client<'_> {
    clients::static_data::Client::new(self)
  }

  /// Returns a client for in-game UI action endpoints.
  pub fn ui(&self) -> clients::ui::Client<'_> {
    clients::ui::Client::new(self)
  }

  /// Returns a client for universe endpoints.
  pub fn universe(&self) -> clients::universe::Client<'_> {
    clients::universe::Client::new(self)
  }

  /// Returns a client for the given war.
  pub fn war(&self, id: i64) -> clients::war::Client<'_> {
    clients::war::Client::new(self, id)
  }

  /// Returns the current EVE server status.
  pub async fn status(&self) -> Result<models::status::ServerStatus, Error> {
    self
      .http()
      .get_json(&self.url_builder().path("v2/status/".to_string()).build(), None)
      .await
  }
}

/// Builder for constructing an ESI [`Client`].
#[derive(Debug, Validate)]
pub struct ClientBuilder {
  base_url: String,
  cache: CacheType,
  #[validate(length(min = 1, message = "client_id must not be empty"))]
  client_id: String,
}

impl ClientBuilder {
  /// Creates a new builder with the given EVE SSO client ID.
  pub fn new(client_id: impl Into<String>) -> Self {
    Self {
      base_url: "https://esi.evetech.net/latest".to_string(),
      cache: CacheType::Memory,
      client_id: client_id.into(),
    }
  }

  /// Overrides the ESI base URL (useful for testing).
  pub fn base_url(mut self, url: impl Into<String>) -> Self {
    self.base_url = url.into();
    self
  }

  /// Sets the cache backend to disk-based storage at the given path.
  pub fn disk_cache(mut self, path: impl Into<std::path::PathBuf>) -> Self {
    self.cache = CacheType::Disk(path.into());
    self
  }

  /// Builds the [`Client`], returning an error if validation fails.
  pub fn build(self) -> Result<Client, Error> {
    self.validate().map_err(|e| Error::Internal(e.to_string()))?;
    let user_agent = format!(
      "Pod/{} ({}; {}; +{})",
      env!("CARGO_PKG_VERSION"),
      std::env::consts::OS,
      std::env::consts::ARCH,
      env!("CARGO_PKG_REPOSITORY")
    );
    let inner = reqwest::Client::builder()
      .user_agent(user_agent)
      .timeout(std::time::Duration::from_secs(30))
      .build()
      .map_err(Error::Http)?;
    let cache = match self.cache {
      CacheType::Disk(path) => CacheStore::Disk(cache::DiskStore::new(path)),
      CacheType::Memory => CacheStore::Memory(cache::MemoryStore::default()),
    };
    let rate_limiter = Arc::new(http::RateLimiter::new());
    let http = http::Client::new(cache, inner, rate_limiter);
    Ok(Client {
      base_url: self.base_url,
      client_id: self.client_id,
      http,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod client_builder {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_builds_with_valid_client_id() {
      let client = ClientBuilder::new("my-client-id").build();
      assert!(client.is_ok());
      assert_eq!(client.unwrap().id(), "my-client-id");
    }

    #[test]
    fn it_rejects_empty_client_id() {
      let result = ClientBuilder::new("").build();
      assert!(result.is_err());
    }

    #[test]
    fn it_uses_default_base_url() {
      let client = ClientBuilder::new("client").build().unwrap();
      assert_eq!(client.base_url, "https://esi.evetech.net/latest");
    }

    #[test]
    fn it_overrides_base_url() {
      let client = ClientBuilder::new("client")
        .base_url("http://localhost:8080")
        .build()
        .unwrap();
      assert_eq!(client.base_url, "http://localhost:8080");
    }
  }
}
