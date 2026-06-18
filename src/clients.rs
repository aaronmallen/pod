use std::sync::LazyLock;

use crate::store;

pub mod esi;
pub mod eve_image;
pub mod eve_sso;
pub mod http;
pub mod muta_market;
pub mod sde;
pub mod zkillboard;

static USER_AGENT: LazyLock<String> = LazyLock::new(|| {
  format!(
    "Pod/{} ({}; {}; +{}; +{})",
    env!("CARGO_PKG_VERSION"),
    std::env::consts::OS,
    std::env::consts::ARCH,
    env!("CARGO_PKG_HOMEPAGE"),
    env!("CARGO_PKG_REPOSITORY"),
  )
});

// `ErrorLimited` keeps EVE's own term for the HTTP 420 error-rate-limit response (distinct from the 429
// `RateLimit`); renaming to drop the `Error` prefix would obscure that domain meaning.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("auth error: {0}")]
  Auth(String),
  #[error("database error: {0}")]
  Db(#[from] store::Error),
  #[error("error limited; reset after {reset_secs}s")]
  ErrorLimited { reset_secs: u64 },
  #[error("http error: {0}")]
  Http(#[from] reqwest::Error),
  #[error("internal error: {0}")]
  Internal(String),
  #[error("json error: {0}")]
  Json(#[from] serde_json::Error),
  #[error("not ready; parent record absent")]
  NotReady,
  #[error("rate limited; retry after {retry_after_secs}s")]
  RateLimit { retry_after_secs: u64 },
}

impl Error {
  pub fn is_foreign_key_violation(&self) -> bool {
    matches!(self, Error::Db(error) if error.is_foreign_key_violation())
  }
}

pub fn user_agent() -> &'static str {
  &USER_AGENT
}
