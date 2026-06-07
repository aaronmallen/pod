use chrono::Utc;
use getset::{CopyGetters, Getters};
use sqlx::FromRow;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  body: Vec<u8>,
  #[getset(get_copy = "pub")]
  cached_at: i64,
  #[getset(get = "pub")]
  etag: Option<String>,
  #[getset(get_copy = "pub")]
  expires_at: Option<i64>,
  #[getset(get = "pub")]
  url: String,
}

impl Model {
  pub fn new(body: Vec<u8>, cached_at: i64, url: impl Into<String>) -> Self {
    Self {
      body,
      cached_at,
      etag: None,
      expires_at: None,
      url: url.into(),
    }
  }

  pub fn is_expired(&self) -> bool {
    self
      .expires_at
      .is_some_and(|expires_at| expires_at < Utc::now().timestamp())
  }

  pub fn set_etag(&mut self, etag: impl Into<String>) {
    self.etag = Some(etag.into());
  }

  pub fn set_expires_at(&mut self, expires_at: i64) {
    self.expires_at = Some(expires_at);
  }
}
