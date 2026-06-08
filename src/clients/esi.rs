use std::sync::Arc;

use serde::{Serialize, de::DeserializeOwned};

use crate::clients::{self, eve_sso::Grant, http};

pub mod alliance;
pub mod bloodlines;
pub mod character;
pub mod corporation;
pub mod dogma;
pub mod faction;
pub mod killmail;
pub mod market;
pub mod models;
pub mod races;
pub mod scopes;
pub mod universe;

const BASE_URL: &str = "https://esi.evetech.net";
/// The pinned ESI compatibility date sent as `X-Compatibility-Date` on every ESI request, replacing the
/// deprecated `vN/` route-version prefixes. Bump this deliberately and in isolation — only after verifying the
/// route deserializers still match the newer date's response shapes. Never derive it from the build date or "today".
const COMPATIBILITY_DATE: &str = "2026-06-08";

pub struct ClientBuilder {
  http: Arc<http::Client>,
  user_agent: String,
}

impl ClientBuilder {
  pub fn build(self) -> Result<Client, clients::Error> {
    if self.user_agent.is_empty() {
      return Err(clients::Error::Auth("user_agent is required".into()));
    }
    Ok(Client {
      base_url: BASE_URL.to_owned(),
      http: self.http,
    })
  }

  pub fn user_agent(mut self, agent: impl Into<String>) -> Self {
    self.user_agent = agent.into();
    self
  }

  pub fn new(http: Arc<http::Client>) -> Self {
    Self {
      http,
      user_agent: String::new(),
    }
  }
}

pub struct Client {
  base_url: String,
  http: Arc<http::Client>,
}

impl Client {
  pub fn builder(http: Arc<http::Client>) -> ClientBuilder {
    ClientBuilder::new(http)
  }

  #[cfg(test)]
  pub fn with_base_url(http: Arc<http::Client>, base_url: impl Into<String>) -> Self {
    Self {
      base_url: base_url.into(),
      http,
    }
  }

  pub fn alliance(&self) -> alliance::Client<'_> {
    alliance::Client::new(self)
  }

  pub fn bloodlines(&self) -> bloodlines::Client<'_> {
    bloodlines::Client::new(self)
  }

  pub fn character(&self) -> character::PublicClient<'_> {
    character::PublicClient::new(self)
  }

  pub fn character_authenticated<'a>(&'a self, grant: &'a Grant) -> character::AuthenticatedClient<'a> {
    character::AuthenticatedClient::new(self, grant)
  }

  pub fn corporation(&self) -> corporation::PublicClient<'_> {
    corporation::PublicClient::new(self)
  }

  pub fn corporation_authenticated<'a>(&'a self, grant: &'a Grant) -> corporation::AuthenticatedClient<'a> {
    corporation::AuthenticatedClient::new(self, grant)
  }

  pub fn dogma(&self) -> dogma::Client<'_> {
    dogma::Client::new(self)
  }

  pub fn faction(&self) -> faction::Client<'_> {
    faction::Client::new(self)
  }

  pub fn killmail(&self) -> killmail::Client<'_> {
    killmail::Client::new(self)
  }

  pub fn market(&self) -> market::Client<'_> {
    market::Client::new(self)
  }

  pub fn races(&self) -> races::Client<'_> {
    races::Client::new(self)
  }

  pub fn universe(&self) -> universe::Client<'_> {
    universe::Client::new(self)
  }

  pub fn http(&self) -> Arc<http::Client> {
    Arc::clone(&self.http)
  }

  pub fn url(&self, path: &str) -> String {
    format!("{}/{}", self.base_url.trim_end_matches('/'), path)
  }

  pub async fn get_json<T: DeserializeOwned>(&self, url: &str, token: Option<&str>) -> Result<T, clients::Error> {
    self.http.get_json(url, token, Some(COMPATIBILITY_DATE)).await
  }

  pub async fn get_json_paginated<T: DeserializeOwned + Send + 'static>(
    &self,
    url: &str,
    token: Option<&str>,
  ) -> Result<Vec<T>, clients::Error> {
    self.http.get_json_paginated(url, token, Some(COMPATIBILITY_DATE)).await
  }

  pub async fn post_json<B: Serialize, T: DeserializeOwned>(
    &self,
    url: &str,
    body: &B,
    token: &str,
  ) -> Result<T, clients::Error> {
    self.http.post_json(url, body, token, Some(COMPATIBILITY_DATE)).await
  }

  pub async fn post_json_anon<B: Serialize, T: DeserializeOwned>(
    &self,
    url: &str,
    body: &B,
  ) -> Result<T, clients::Error> {
    self.http.post_json_anon(url, body, Some(COMPATIBILITY_DATE)).await
  }

  pub async fn put_empty<B: Serialize>(&self, url: &str, body: &B, token: &str) -> Result<(), clients::Error> {
    self.http.put_empty(url, body, token, Some(COMPATIBILITY_DATE)).await
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  async fn make_http() -> Arc<http::Client> {
    let db = store::open_test().await.unwrap();
    let cache = http::Cache::new(db);
    http::Client::builder(cache).build()
  }

  mod client {
    use super::*;

    mod builder {
      use super::*;

      #[tokio::test]
      async fn it_returns_err_when_user_agent_is_empty() {
        let http = make_http().await;

        let result = Client::builder(http).build();

        assert!(result.is_err());
      }

      #[tokio::test]
      async fn it_returns_ok_when_user_agent_is_set() {
        let http = make_http().await;

        let result = Client::builder(http).user_agent("Pod/1.0").build();

        assert!(result.is_ok());
      }
    }
  }
}
