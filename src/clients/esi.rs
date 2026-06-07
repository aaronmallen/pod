use std::sync::Arc;

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

  pub fn http(&self) -> &http::Client {
    &self.http
  }

  pub fn url(&self, path: &str) -> String {
    format!("{}/{}", self.base_url.trim_end_matches('/'), path)
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
