//! Fire-and-forget telemetry transport, identity, and baked endpoint config
//! (spec mmmzstpq §7.2, §7.3, §7.5).
//!
//! This is a standalone, single-shot POST sender — deliberately *not* built on
//! [`crate::clients::http::Client`]. The whole subsystem fails closed and
//! silently: if the build-time endpoint config is absent (every local / dev
//! build), [`Endpoint::from_env`] returns `None` and nothing is ever sent;
//! every send outcome is swallowed into a `trace!` / `debug!` under the
//! `pod::telemetry` target and is never surfaced to the UI.
//!
//! The wire shape is the frozen canonical contract in
//! [`crate::services::telemetry::contract`]; this module reuses [`Batch`] verbatim and adds
//! only identity derivation, endpoint resolution, and the reqwest sender.

use std::{sync::Arc, time::Duration};

use sha2::{Digest, Sha256};

use crate::services::telemetry::contract::Batch;

/// The header carrying the per-deployment write key (§7.3). The Worker
/// authenticates each ingest POST against the rotating key set on this header.
const WRITE_KEY_HEADER: &str = "X-Pod-Telemetry-Key";

const SEND_TIMEOUT: Duration = Duration::from_secs(10);

/// Minimal static user agent (§7.3). Deliberately content-free beyond the
/// product + version — no OS / arch / locale (those ride the environment
/// stream when, and only when, the user has opted in).
const USER_AGENT: &str = concat!("Pod/", env!("CARGO_PKG_VERSION"), " (telemetry)");

/// `anon_id(machine_id)` = lowercase hex sha256 of the machine id (§7.2).
///
/// Derived on the fly at send (and at preview render) from the process-cached
/// machine id; it is NEVER persisted anywhere. `anon_id("")` is a valid 64-char
/// hash and never panics or returns blank.
pub fn anon_id(machine_id: &str) -> String {
  let digest = Sha256::digest(machine_id.as_bytes());
  digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Clone, Debug)]
pub struct Endpoint {
  pub url: String,
  pub key: String,
}

impl Endpoint {
  pub fn from_env() -> Option<Self> {
    let url = option_env!("POD_TELEMETRY_URL")?.trim().to_owned();
    if url.is_empty() {
      return None;
    }
    let key = option_env!("POD_TELEMETRY_KEY")?.trim().to_owned();
    if key.is_empty() {
      return None;
    }
    Some(Self {
      url,
      key,
    })
  }
}

#[derive(Clone)]
pub struct Sender {
  client: reqwest::Client,
  endpoint: Arc<Endpoint>,
}

impl Sender {
  pub fn new(endpoint: Endpoint) -> Option<Self> {
    let client = reqwest::Client::builder()
      .user_agent(USER_AGENT)
      .timeout(SEND_TIMEOUT)
      .build()
      .ok()?;
    Some(Self {
      client,
      endpoint: Arc::new(endpoint),
    })
  }

  /// POST one [`Batch`] to the ingest endpoint with the write-key header and
  /// return whether the server accepted it (2xx).
  ///
  /// Every outcome — serialize failure, transport error, non-2xx status — is
  /// swallowed into a `trace!` / `debug!` under `target: "pod::telemetry"` and
  /// reported as `false`; nothing ever bubbles out as a `Message` / toast /
  /// modal. The crash path uses the returned bool to decide whether to delete
  /// its disk buffer; the session path ignores it.
  pub async fn send(&self, batch: &Batch) -> bool {
    let body = match serde_json::to_vec(batch) {
      Ok(body) => body,
      Err(error) => {
        tracing::debug!(target: "pod::telemetry", %error, "failed to serialize telemetry batch");
        return false;
      }
    };

    let response = self
      .client
      .post(&self.endpoint.url)
      .header(WRITE_KEY_HEADER, &self.endpoint.key)
      .header(reqwest::header::CONTENT_TYPE, "application/json")
      .body(body)
      .send()
      .await;

    match response {
      Ok(response) => {
        let status = response.status();
        if status.is_success() {
          tracing::trace!(target: "pod::telemetry", %status, "telemetry batch accepted");
          true
        } else {
          tracing::debug!(target: "pod::telemetry", %status, "telemetry batch rejected");
          false
        }
      }
      Err(error) => {
        tracing::debug!(target: "pod::telemetry", %error, "telemetry POST failed");
        false
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::services::telemetry::contract::SESSION_ALL_STREAMS_FIXTURE;

  #[test]
  fn batch_serializes_byte_for_byte_to_the_golden_session_fixture() {
    let batch: Batch = serde_json::from_str(SESSION_ALL_STREAMS_FIXTURE).unwrap();
    let reserialized = serde_json::to_string_pretty(&batch).unwrap();
    assert_eq!(reserialized, SESSION_ALL_STREAMS_FIXTURE.trim_end());
  }

  #[test]
  fn anon_id_is_lowercase_hex_sha256_of_the_machine_id() {
    assert_eq!(
      anon_id(""),
      "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
  }

  #[test]
  fn anon_id_is_64_lowercase_hex_chars_and_never_panics_on_empty() {
    let id = anon_id("");
    assert_eq!(id.len(), 64);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
  }

  #[test]
  fn anon_id_is_stable_and_distinguishes_inputs() {
    assert_eq!(anon_id("machine-a"), anon_id("machine-a"));
    assert_ne!(anon_id("machine-a"), anon_id("machine-b"));
  }

  #[test]
  fn endpoint_from_env_is_none_when_build_time_vars_absent() {
    assert!(Endpoint::from_env().is_none());
  }

  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
  };

  fn sender_for(url: &str, key: &str) -> Sender {
    Sender::new(Endpoint {
      url: url.to_owned(),
      key: key.to_owned(),
    })
    .expect("reqwest client builds")
  }

  fn fixture_batch() -> Batch {
    serde_json::from_str(SESSION_ALL_STREAMS_FIXTURE).expect("golden fixture parses")
  }

  #[tokio::test]
  async fn send_posts_the_batch_with_the_write_key_header_and_returns_true_on_2xx() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
      .and(path("/ingest"))
      .and(header(WRITE_KEY_HEADER, "secret-key"))
      .and(header(reqwest::header::CONTENT_TYPE.as_str(), "application/json"))
      .respond_with(ResponseTemplate::new(202))
      .expect(1)
      .mount(&server)
      .await;

    let sender = sender_for(&format!("{}/ingest", server.uri()), "secret-key");
    let accepted = sender.send(&fixture_batch()).await;

    assert!(accepted, "a 2xx ingest response is reported as accepted");
  }

  #[tokio::test]
  async fn send_returns_false_on_a_non_2xx_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
      .respond_with(ResponseTemplate::new(500))
      .mount(&server)
      .await;

    let sender = sender_for(&server.uri(), "key");
    let accepted = sender.send(&fixture_batch()).await;

    assert!(!accepted, "a rejected (non-2xx) batch is reported as not accepted");
  }

  #[tokio::test]
  async fn send_swallows_a_transport_error_and_returns_false() {
    let sender = sender_for("http://127.0.0.1:1/ingest", "key");
    let accepted = sender.send(&fixture_batch()).await;

    assert!(
      !accepted,
      "a transport failure is swallowed and reported as not accepted"
    );
  }
}
