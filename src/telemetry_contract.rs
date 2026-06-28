//! Frozen golden telemetry wire contract (spec mmmzstpq §6).
//!
//! This module is the canonical, source-of-truth Rust representation of the
//! telemetry wire shape: the exact JSON the client sends and the Cloudflare
//! Worker / D1 backend accepts. It is the SHARED CONTRACT that downstream
//! components build against:
//!
//! * the Rust sender (`src/clients/telemetry.rs`) serializes a [`Batch`];
//! * the Worker validator (`telemetry/src/contract.ts`) accepts the same shape;
//! * the settings preview (`telemetry_tab.rs`) reproduces it byte-for-byte.
//!
//! The two committed golden fixtures pin the wire shape across all three
//! components so drift fails CI:
//!
//! * [`SESSION_ALL_STREAMS_FIXTURE`] — §6.3, every session stream ON;
//! * [`CRASH_BATCH_FIXTURE`] — §6.4, the disk-buffered crash envelope.
//!
//! Serialization rules baked in here (see §6.1):
//!
//! * disabled / absent streams are OMITTED keys, never `null`
//!   (`skip_serializing_if`);
//! * `app.git_sha` / `app.build_date` are omitted when unset;
//! * a usage event carries `on` ONLY for `feature_toggle`;
//! * field ordering matches the canonical envelope (schema, kind, id, session,
//!   app, sent_at, streams), and the fixtures are pretty-printed (two-space
//!   indent) so a `serde_json::to_string_pretty` round-trip is byte-stable.

use serde::{Deserialize, Serialize};

/// The §6.3 golden session envelope (all streams ON), verbatim.
///
/// Shared with the TypeScript Worker tests via the same path
/// (`tests/fixtures/telemetry/session_all_streams.json`).
// Golden fixture pinned by the contract tests; no production reader yet.
#[cfg_attr(not(test), expect(dead_code))]
pub const SESSION_ALL_STREAMS_FIXTURE: &str = include_str!("../tests/fixtures/telemetry/session_all_streams.json");

/// The §6.4 golden crash envelope, verbatim.
///
/// Shared with the TypeScript Worker tests via the same path
/// (`tests/fixtures/telemetry/crash_batch.json`).
// Golden fixture pinned by the contract tests; no production reader yet.
#[cfg_attr(not(test), expect(dead_code))]
pub const CRASH_BATCH_FIXTURE: &str = include_str!("../tests/fixtures/telemetry/crash_batch.json");

/// The integer contract version baked into every envelope (`schema`). The Worker
/// rejects (400) any envelope whose `schema` exceeds the version it knows.
pub const SCHEMA_VERSION: u32 = 1;

/// The Worker's session-tag regex, mirrored for client-side conformance tests:
/// `^s_[0-9a-f]{8}$`. A crash envelope's `session` MUST satisfy it (§6.4).
// Conformance regex asserted by the contract tests; no production reader yet.
#[cfg_attr(not(test), expect(dead_code))]
pub const SESSION_TAG_PATTERN: &str = r"^s_[0-9a-f]{8}$";

/// The Worker's anon-id regex (`^[0-9a-f]{64}$`): lowercase sha256 hex (§6.1).
// Conformance regex asserted by the contract tests; no production reader yet.
#[cfg_attr(not(test), expect(dead_code))]
pub const ANON_ID_PATTERN: &str = r"^[0-9a-f]{64}$";

/// The batch kind discriminator. A POST is exactly one envelope of one kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
  /// A `kind:"session"` batch — any subset of usage / performance / environment
  /// (never crashes).
  Session,
  /// A `kind:"crash"` batch — exactly the `crashes` stream, disk-buffered and
  /// sent on the next launch.
  Crash,
}

/// The kind of a single usage event (§6.1.1, §8.1). Closed enum; `name` is
/// always a fixed token, never free text.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageEventKind {
  /// A view/route open at the central `navigate()` dispatcher.
  ViewOpen,
  /// A feature toggle persisted in settings (carries `on`).
  FeatureToggle,
  /// A sub-section selection at the central dispatcher.
  SubSection,
}

/// App identity carried on every envelope (§6.1). `git_sha` / `build_date` are
/// omitted entirely when their build-time values are unset.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct App {
  /// `env!("CARGO_PKG_VERSION")`.
  pub version: String,
  /// `option_env!("POD_GIT_SHA")`; omitted if unset.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub git_sha: Option<String>,
  /// `option_env!("POD_BUILD_DATE")`; omitted if unset.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub build_date: Option<String>,
}

/// A single usage event (§6.1.1). `on` is present iff `kind` is
/// [`UsageEventKind::FeatureToggle`].
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct UsageEvent {
  /// RFC3339 UTC timestamp of the event.
  pub t: String,
  /// The event kind (closed enum).
  pub kind: UsageEventKind,
  /// The route / feature token (fixed, parameter-free; never free text).
  pub name: String,
  /// The toggle state — present only for `feature_toggle`.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub on: Option<bool>,
}

/// The usage stream (§6.1.1): a list of usage events.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct UsageStream {
  /// The captured usage events.
  pub events: Vec<UsageEvent>,
}

/// The performance stream (§6.1.1): per-view timings plus one batch-level heap
/// snapshot.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PerformanceStream {
  /// One entry per navigation in the batch.
  pub views: Vec<PerformanceViewEntry>,
  /// Single batch-level live-heap snapshot in mebibytes.
  pub heap_mb: u64,
}

/// A performance view entry: route token plus its timings (§6.1.1).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PerformanceViewEntry {
  /// The route token (same vocabulary as usage `name`).
  pub name: String,
  /// Nav→first-paint time in milliseconds.
  pub load_ms: u64,
  /// Frame-time p95 in milliseconds.
  pub frame_p95_ms: u64,
}

/// The environment stream (§6.1.1). Closed-world: exactly these six string
/// fields, each the literal `"unknown"` when unresolvable (never omitted).
/// `pod_version` is intentionally absent — it duplicated [`App::version`] (§5.3).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvironmentStream {
  /// `std::env::consts::OS`.
  pub os: String,
  /// Major OS version only (e.g. `"15"`), or `"unknown"`.
  pub os_version: String,
  /// `std::env::consts::ARCH`.
  pub arch: String,
  /// Primary window logical size `"WxH"`, or `"unknown"`.
  pub window_size: String,
  /// Primary monitor logical size `"WxH"`, or `"unknown"`.
  pub screen_size: String,
  /// Language-only locale (region subtag dropped), or `"unknown"`.
  pub locale: String,
}

/// A single crash report (§6.4). All free-text fields are scrubbed per §5.4/§5.5
/// before they ever reach this struct.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CrashReport {
  /// When the panic happened (from the disk buffer, never wall-clock-now).
  pub crashed_at: String,
  /// The (scrubbed, truncated) panic message.
  pub message: String,
  /// App-root-relative panic location; omitted if unavailable.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub location: Option<String>,
  /// Scrubbed backtrace frame strings; omitted if empty/unavailable.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub backtrace: Option<Vec<String>>,
  /// Allow-listed, scrubbed log lines; omitted if empty/unavailable.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub context_log: Option<Vec<String>>,
}

/// The crashes stream (§6.4): one or more crash reports in a `kind:"crash"`
/// batch.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CrashStream {
  /// The buffered crash reports.
  pub reports: Vec<CrashReport>,
}

/// The `streams` object (§6.1.1 / §6.4). A session batch carries any subset of
/// usage / performance / environment; a crash batch carries exactly `crashes`.
/// Disabled / absent streams are OMITTED keys (never `null`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Streams {
  /// Usage stream (session batches only).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub usage: Option<UsageStream>,
  /// Performance stream (session batches only).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub performance: Option<PerformanceStream>,
  /// Environment stream (session batches only; first flush only).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub environment: Option<EnvironmentStream>,
  /// Crashes stream (crash batches only).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub crashes: Option<CrashStream>,
}

/// One telemetry envelope = one POST (§6.1). This is the frozen top-level wire
/// shape both the session (§6.3) and crash (§6.4) golden fixtures conform to.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Batch {
  /// Integer contract version; the Worker rejects unknown values.
  pub schema: u32,
  /// `"session"` or `"crash"`.
  pub kind: Kind,
  /// `sha256(machine_id)` hex, lowercase, 64 chars; derived at send, never
  /// stored.
  pub id: String,
  /// `"s_" + 8 lowercase hex`, once per process; content-free.
  pub session: String,
  /// App identity.
  pub app: App,
  /// RFC3339 UTC flush / send time.
  pub sent_at: String,
  /// The enabled streams for this batch.
  pub streams: Streams,
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;
  use serde_json::Value;

  use super::*;

  fn session_all_streams() -> Batch {
    Batch {
      schema: 1,
      kind: Kind::Session,
      id: "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08".to_string(),
      session: "s_1a2b3c4d".to_string(),
      app: App {
        version: "0.9.4".to_string(),
        git_sha: Some("2364bc8c".to_string()),
        build_date: Some("2026-06-20".to_string()),
      },
      sent_at: "2026-06-25T14:32:08Z".to_string(),
      streams: Streams {
        usage: Some(UsageStream {
          events: vec![
            UsageEvent {
              t: "2026-06-25T14:30:01Z".to_string(),
              kind: UsageEventKind::ViewOpen,
              name: "wallet".to_string(),
              on: None,
            },
            UsageEvent {
              t: "2026-06-25T14:31:02Z".to_string(),
              kind: UsageEventKind::FeatureToggle,
              name: "skills.plan_optimizer".to_string(),
              on: Some(true),
            },
          ],
        }),
        performance: Some(PerformanceStream {
          views: vec![PerformanceViewEntry {
            name: "wallet".to_string(),
            load_ms: 142,
            frame_p95_ms: 11,
          }],
          heap_mb: 84,
        }),
        environment: Some(EnvironmentStream {
          os: "macos".to_string(),
          os_version: "15".to_string(),
          arch: "aarch64".to_string(),
          window_size: "2560x1440".to_string(),
          screen_size: "3440x1440".to_string(),
          locale: "en".to_string(),
        }),
        crashes: None,
      },
    }
  }

  fn crash_batch() -> Batch {
    Batch {
      schema: 1,
      kind: Kind::Crash,
      id: "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08".to_string(),
      session: "s_9f2a1b3c".to_string(),
      app: App {
        version: "0.9.3".to_string(),
        git_sha: Some("b241311d".to_string()),
        build_date: Some("2026-06-18".to_string()),
      },
      sent_at: "2026-06-25T08:01:55Z".to_string(),
      streams: Streams {
        usage: None,
        performance: None,
        environment: None,
        crashes: Some(CrashStream {
          reports: vec![CrashReport {
            crashed_at: "2026-06-24T22:14:03Z".to_string(),
            message: "called `Option::unwrap()` on a `None` value".to_string(),
            location: Some("src/features/wallet.rs:412".to_string()),
            backtrace: Some(vec![
              "pod::features::wallet::reconcile".to_string(),
              "pod::app::update".to_string(),
              "iced_runtime::program::run".to_string(),
            ]),
            context_log: Some(vec![
              r#"{"level":"INFO","target":"pod::nav","message":"navigated"}"#.to_string(),
              r#"{"level":"INFO","target":"pod::updater","message":"checked for updates"}"#.to_string(),
            ]),
          }],
        }),
      },
    }
  }

  #[test]
  fn session_serializes_byte_for_byte_to_the_golden_fixture() {
    let serialized = serde_json::to_string_pretty(&session_all_streams()).unwrap();
    assert_eq!(serialized, SESSION_ALL_STREAMS_FIXTURE.trim_end());
  }

  #[test]
  fn crash_serializes_byte_for_byte_to_the_golden_fixture() {
    let serialized = serde_json::to_string_pretty(&crash_batch()).unwrap();
    assert_eq!(serialized, CRASH_BATCH_FIXTURE.trim_end());
  }

  #[test]
  fn session_fixture_round_trips_through_the_contract_types() {
    let parsed: Batch = serde_json::from_str(SESSION_ALL_STREAMS_FIXTURE).unwrap();
    assert_eq!(parsed, session_all_streams());
    let reserialized = serde_json::to_string_pretty(&parsed).unwrap();
    assert_eq!(reserialized, SESSION_ALL_STREAMS_FIXTURE.trim_end());
  }

  #[test]
  fn crash_fixture_round_trips_through_the_contract_types() {
    let parsed: Batch = serde_json::from_str(CRASH_BATCH_FIXTURE).unwrap();
    assert_eq!(parsed, crash_batch());
    let reserialized = serde_json::to_string_pretty(&parsed).unwrap();
    assert_eq!(reserialized, CRASH_BATCH_FIXTURE.trim_end());
  }

  #[test]
  fn session_fixture_is_valid_json_with_the_pinned_envelope_keys() {
    let value: Value = serde_json::from_str(SESSION_ALL_STREAMS_FIXTURE).unwrap();
    let obj = value.as_object().unwrap();
    assert_eq!(obj["schema"], Value::from(1));
    assert_eq!(obj["kind"], Value::from("session"));
    let streams = obj["streams"].as_object().unwrap();
    for key in streams.keys() {
      assert!(
        matches!(key.as_str(), "usage" | "performance" | "environment"),
        "unexpected session stream key: {key}"
      );
    }
    assert!(!streams.contains_key("crashes"));
  }

  #[test]
  fn crash_fixture_streams_object_holds_exactly_the_crashes_key() {
    let value: Value = serde_json::from_str(CRASH_BATCH_FIXTURE).unwrap();
    let streams = value["streams"].as_object().unwrap();
    let keys: Vec<&String> = streams.keys().collect();
    assert_eq!(keys, vec!["crashes"]);
  }

  #[test]
  fn disabled_streams_are_omitted_keys_never_null() {
    let mut batch = session_all_streams();
    batch.streams.usage = None;
    let value: Value = serde_json::to_value(&batch).unwrap();
    let streams = value["streams"].as_object().unwrap();
    assert!(!streams.contains_key("usage"));
    assert!(streams.contains_key("performance"));
    assert!(streams.contains_key("environment"));
  }

  #[test]
  fn usage_view_open_omits_on_while_feature_toggle_carries_it() {
    let value: Value = serde_json::to_value(session_all_streams()).unwrap();
    let events = value["streams"]["usage"]["events"].as_array().unwrap();
    assert!(!events[0].as_object().unwrap().contains_key("on"));
    assert_eq!(events[1]["on"], Value::Bool(true));
  }

  #[test]
  fn crash_fixture_session_matches_the_worker_session_regex() {
    let value: Value = serde_json::from_str(CRASH_BATCH_FIXTURE).unwrap();
    let session = value["session"].as_str().unwrap();
    assert!(
      is_session_tag(session),
      "session {session} must match {SESSION_TAG_PATTERN}"
    );
  }

  #[test]
  fn both_fixture_ids_match_the_worker_anon_id_regex() {
    for fixture in [SESSION_ALL_STREAMS_FIXTURE, CRASH_BATCH_FIXTURE] {
      let value: Value = serde_json::from_str(fixture).unwrap();
      let id = value["id"].as_str().unwrap();
      assert!(is_anon_id(id), "id {id} must match {ANON_ID_PATTERN}");
    }
  }

  #[test]
  fn unset_app_git_sha_and_build_date_are_omitted() {
    let mut batch = session_all_streams();
    batch.app.git_sha = None;
    batch.app.build_date = None;
    let value: Value = serde_json::to_value(&batch).unwrap();
    let app = value["app"].as_object().unwrap();
    assert!(!app.contains_key("git_sha"));
    assert!(!app.contains_key("build_date"));
    assert!(app.contains_key("version"));
  }

  fn is_session_tag(s: &str) -> bool {
    s.len() == 10 && s.starts_with("s_") && s[2..].chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
  }

  fn is_anon_id(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
  }
}

#[cfg(test)]
mod conformance {
  use pretty_assertions::assert_eq;
  use serde_json::Value;

  use super::*;

  const WORKER_CONTRACT_TS: &str = include_str!("../telemetry/src/contract.ts");
  const D1_INIT_SQL: &str = include_str!("../telemetry/migrations/0001_init.sql");

  fn settings_preview_batch() -> Batch {
    Batch {
      schema: SCHEMA_VERSION,
      kind: Kind::Session,
      id: "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08".to_string(),
      session: "s_1a2b3c4d".to_string(),
      app: App {
        version: "0.9.4".to_string(),
        git_sha: Some("2364bc8c".to_string()),
        build_date: Some("2026-06-20".to_string()),
      },
      sent_at: "2026-06-25T14:32:08Z".to_string(),
      streams: Streams {
        usage: Some(UsageStream {
          events: vec![
            UsageEvent {
              t: "2026-06-25T14:30:01Z".to_string(),
              kind: UsageEventKind::ViewOpen,
              name: "wallet".to_string(),
              on: None,
            },
            UsageEvent {
              t: "2026-06-25T14:31:02Z".to_string(),
              kind: UsageEventKind::FeatureToggle,
              name: "skills.plan_optimizer".to_string(),
              on: Some(true),
            },
          ],
        }),
        performance: Some(PerformanceStream {
          views: vec![PerformanceViewEntry {
            name: "wallet".to_string(),
            load_ms: 142,
            frame_p95_ms: 11,
          }],
          heap_mb: 84,
        }),
        environment: Some(EnvironmentStream {
          os: "macos".to_string(),
          os_version: "15".to_string(),
          arch: "aarch64".to_string(),
          window_size: "2560x1440".to_string(),
          screen_size: "3440x1440".to_string(),
          locale: "en".to_string(),
        }),
        crashes: None,
      },
    }
  }

  fn field_names(value: &Value, into: &mut Vec<String>) {
    match value {
      Value::Object(map) => {
        for (key, child) in map {
          into.push(key.clone());
          field_names(child, into);
        }
      }
      Value::Array(items) => {
        for item in items {
          field_names(item, into);
        }
      }
      _ => {}
    }
  }

  #[test]
  fn rust_client_batch_matches_the_golden_session_fixture() {
    let serialized = serde_json::to_string_pretty(&settings_preview_batch()).unwrap();
    assert_eq!(serialized, SESSION_ALL_STREAMS_FIXTURE.trim_end());
  }

  #[test]
  fn settings_preview_reproduces_the_golden_session_field_names() {
    let preview: Value = serde_json::to_value(settings_preview_batch()).unwrap();
    let golden: Value = serde_json::from_str(SESSION_ALL_STREAMS_FIXTURE).unwrap();

    let mut preview_names = Vec::new();
    let mut golden_names = Vec::new();
    field_names(&preview, &mut preview_names);
    field_names(&golden, &mut golden_names);

    assert_eq!(
      preview_names, golden_names,
      "settings preview must reproduce the golden session field shape"
    );
  }

  #[test]
  fn crash_fixture_session_passes_the_worker_session_regex() {
    let value: Value = serde_json::from_str(CRASH_BATCH_FIXTURE).unwrap();
    let session = value["session"].as_str().unwrap();
    assert!(
      matches_session_tag(session),
      "crash session {session} must match {SESSION_TAG_PATTERN}"
    );
    assert!(
      WORKER_CONTRACT_TS.contains("/^s_[0-9a-f]{8}$/"),
      "worker source must enforce the session regex this test mirrors"
    );
  }

  #[test]
  fn pod_version_absent_from_the_rust_payload() {
    let value: Value = serde_json::to_value(settings_preview_batch()).unwrap();
    let env = value["streams"]["environment"].as_object().unwrap();
    assert!(
      !env.contains_key("pod_version"),
      "environment must not carry pod_version"
    );
    assert!(
      !serde_json::to_string(&value).unwrap().contains("pod_version"),
      "no envelope field may be named pod_version"
    );
  }

  #[test]
  fn pod_version_absent_from_the_worker_validator() {
    assert!(
      WORKER_CONTRACT_TS
        .contains(r#"const keys = ["os", "os_version", "arch", "window_size", "screen_size", "locale"];"#),
      "worker environment allow-list must be the closed six fields (no pod_version)"
    );
    assert!(
      !WORKER_CONTRACT_TS.contains("pod_version"),
      "worker contract source must not name pod_version as an accepted field"
    );
  }

  #[test]
  fn pod_version_absent_from_the_d1_schema() {
    assert!(
      !D1_INIT_SQL.contains("pod_version"),
      "D1 schema must not declare a pod_version column"
    );
    assert!(D1_INIT_SQL.contains("app_version"), "D1 schema keeps app_version");
  }

  #[test]
  fn pod_version_absent_from_the_settings_preview() {
    let value: Value = serde_json::to_value(settings_preview_batch()).unwrap();
    let mut names = Vec::new();
    field_names(&value, &mut names);
    assert!(
      !names.iter().any(|name| name == "pod_version"),
      "settings preview must not surface pod_version"
    );
  }

  fn matches_session_tag(s: &str) -> bool {
    s.len() == 10 && s.starts_with("s_") && s[2..].chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
  }
}
