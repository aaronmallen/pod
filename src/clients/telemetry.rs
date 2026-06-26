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
//! [`crate::telemetry_contract`]; this module reuses [`Batch`] verbatim and adds
//! only identity derivation, endpoint resolution, and the reqwest sender.

// Transport surface consumed by the §7.4 collector / flush loop
// (`src/services/telemetry.rs`), which lands in a sibling task. These items are
// exercised by this module's own tests but have no production reader yet, so the
// unused warnings are expected until that wiring lands.
#![allow(dead_code)]

use std::{sync::Arc, time::Duration};

use sha2::{Digest, Sha256};

use crate::telemetry_contract::Batch;

/// The header carrying the per-deployment write key (§7.3). The Worker
/// authenticates each ingest POST against the rotating key set on this header.
const WRITE_KEY_HEADER: &str = "X-Pod-Telemetry-Key";

/// Single-shot POST timeout (§7.3). Telemetry is best-effort and must never
/// stall a flush, so the request is capped tightly.
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

/// The build-time-baked ingest endpoint (§7.3, §7.5). Present only in release
/// builds where both `POD_TELEMETRY_URL` and `POD_TELEMETRY_KEY` were injected;
/// absent (and therefore a no-op) everywhere else.
#[derive(Clone, Debug)]
pub struct Endpoint {
  /// The ingest URL (`POD_TELEMETRY_URL`, a literal in release.yml).
  pub url: String,
  /// The write key (`POD_TELEMETRY_KEY`, a CI secret), sent on
  /// [`WRITE_KEY_HEADER`].
  pub key: String,
}

impl Endpoint {
  /// Resolve the endpoint from the build-time environment (§7.3). `Some` only
  /// when both `option_env!("POD_TELEMETRY_URL")` and
  /// `option_env!("POD_TELEMETRY_KEY")` are present and non-blank; else `None`
  /// (the whole subsystem no-ops). Mirrors `updater::Config::from_env`.
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

/// The fire-and-forget telemetry sender (§7.3): a reqwest client plus the baked
/// endpoint. Cheap to clone (the endpoint is `Arc`-shared).
#[derive(Clone)]
pub struct Sender {
  client: reqwest::Client,
  endpoint: Arc<Endpoint>,
}

impl Sender {
  /// Build a sender for a resolved [`Endpoint`]. Returns `None` if the reqwest
  /// client cannot be constructed (e.g. no TLS backend) — another fail-closed
  /// no-op path.
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
  use crate::telemetry_contract::SESSION_ALL_STREAMS_FIXTURE;

  // The sender serializes the canonical contract `Batch`; this asserts the
  // reused types produce the golden session fixture byte-for-byte (compact body
  // matches the pretty fixture once re-pretty-printed), pinning that the
  // transport rides exactly the frozen contract.
  #[test]
  fn batch_serializes_byte_for_byte_to_the_golden_session_fixture() {
    let batch: Batch = serde_json::from_str(SESSION_ALL_STREAMS_FIXTURE).unwrap();
    let reserialized = serde_json::to_string_pretty(&batch).unwrap();
    assert_eq!(reserialized, SESSION_ALL_STREAMS_FIXTURE.trim_end());
  }

  // ---- §7.2 identity: lowercase hex sha256, derived on the fly, never stored.

  #[test]
  fn anon_id_is_lowercase_hex_sha256_of_the_machine_id() {
    // sha256("") — the documented valid empty-input hash.
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

  // ---- §7.3 endpoint: Some only when both vars present & non-blank.

  #[test]
  fn endpoint_from_env_is_none_when_build_time_vars_absent() {
    // These option_env! vars are unset in the test build, so resolution must
    // no-op exactly as it does in every local / dev build.
    assert!(Endpoint::from_env().is_none());
  }
}
