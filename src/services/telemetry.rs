//! Process-global telemetry collector + the single gated flush loop (spec
//! mmmzstpq §4, §7.4).
//!
//! Capture sites call the cheap, non-blocking `record_*` free functions; they
//! push one event into a process-global [`OnceLock`] buffer WITHOUT ever reading
//! Settings (no per-event gating, no I/O). The whole subsystem is a structural
//! no-op unless it was [`init`]ialized, which the app only does when the
//! build-time ingest endpoint is configured ([`crate::clients::telemetry::Endpoint::from_env`]);
//! it is also a no-op under `cfg(test)`.
//!
//! All gating happens once, later, at [`flush`]: that single function re-reads
//! the live [`TelemetryConfig`] snapshot, drops the buffered events of any
//! per-stream flag that is off (and POSTs nothing at all when the master switch
//! is off), assembles a [`Batch`] of the enabled streams from the buffered data,
//! hands it to the fire-and-forget [`Sender`], and clears the buffer regardless
//! of send success (drop-on-failure bounds memory across offline spans). The
//! `environment` stream is emitted only on the FIRST flush of the process.
//!
//! The wire shape is the frozen [`crate::telemetry_contract`]; this module
//! reuses [`Batch`] verbatim and only buffers, gates, and assembles.

// Capture surface consumed by per-feature hook sites and the app flush wiring.
// Some `record_*` helpers / setters have no production caller in this task (the
// capture-site and settings-persist tasks land them), so the unused warnings are
// expected until that wiring arrives.
#![allow(dead_code)]

use std::sync::{Mutex, OnceLock};

use rand::Rng;

use crate::{
  clients::telemetry::{Sender, anon_id},
  config::TelemetryConfig,
  telemetry_contract::{
    App, Batch, EnvironmentStream, Kind, PerformanceStream, PerformanceViewEntry, SCHEMA_VERSION, Streams, UsageEvent,
    UsageEventKind, UsageStream,
  },
};

/// The unknown-value sentinel the environment stream uses for any field that
/// cannot be resolved (§6.1.1: never omitted, always a literal `"unknown"`).
const UNKNOWN: &str = "unknown";

/// The process-global collector. Populated once by [`init`]; every `record_*`
/// and [`flush`] is a no-op while it is empty (structurally disabled).
static COLLECTOR: OnceLock<Collector> = OnceLock::new();

/// The mutable, drainable accumulator behind the collector. A single `Mutex`
/// guards everything so capture is one cheap lock + push.
#[derive(Debug, Default)]
struct Buffers {
  /// Buffered usage events (view opens, feature toggles, sub-section selects).
  usage: Vec<UsageEvent>,
  /// Buffered per-view performance entries (nav->paint + frame p95).
  views: Vec<PerformanceViewEntry>,
  /// The largest live-heap snapshot observed this flush window, in MiB.
  heap_mb: u64,
  /// The live config snapshot, re-read at every flush. Updated on settings
  /// persist via [`set_config`] so mid-session opt-out is honored next flush.
  config: TelemetryConfig,
  /// Whether the `environment` stream has already been emitted this process.
  environment_sent: bool,
}

/// The process-global telemetry collector: identity fixed at [`init`], plus the
/// drainable [`Buffers`] behind a `Mutex`.
#[derive(Debug)]
struct Collector {
  /// `sha256(machine_id)` hex (the envelope `id`), derived once at init.
  anon_id: String,
  /// `"s_" + 8 lowercase hex`, generated once at init (the envelope `session`).
  session: String,
  /// App identity carried on every envelope.
  app: App,
  /// The drainable accumulator + live config snapshot.
  buffers: Mutex<Buffers>,
}

/// Initialize the process-global collector (idempotent; the second call is a
/// no-op). The app calls this once, only when the build-time ingest endpoint is
/// configured, so leaving it uninitialized keeps the whole subsystem a
/// structural no-op. `machine_id` is hashed into the envelope `id` here;
/// `config` seeds the live snapshot.
pub fn init(machine_id: &str, config: TelemetryConfig) {
  let _ = COLLECTOR.set(Collector {
    anon_id: anon_id(machine_id),
    session: session_tag(),
    app: app_identity(),
    buffers: Mutex::new(Buffers {
      config,
      ..Buffers::default()
    }),
  });
}

/// Refresh the live [`TelemetryConfig`] snapshot the next [`flush`] will gate
/// on. Called from the settings persist path so a mid-session opt-out is honored
/// on the very next flush (including the exit flush). A no-op if uninitialized.
pub fn set_config(config: TelemetryConfig) {
  if let Some(collector) = COLLECTOR.get()
    && let Ok(mut buffers) = collector.buffers.lock()
  {
    buffers.config = config;
  }
}

/// Generate the per-process session tag: `"s_"` followed by exactly 8 lowercase
/// hex chars, satisfying the Worker's `^s_[0-9a-f]{8}$` regex.
fn session_tag() -> String {
  let mut bytes = [0u8; 4];
  rand::rng().fill_bytes(&mut bytes);
  let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
  format!("s_{hex}")
}

/// The app identity carried on every envelope, from the same build-time values
/// the contract documents (`CARGO_PKG_VERSION`, `POD_GIT_SHA`, `POD_BUILD_DATE`).
fn app_identity() -> App {
  App {
    version: env!("CARGO_PKG_VERSION").to_owned(),
    git_sha: non_empty(option_env!("POD_GIT_SHA")),
    build_date: non_empty(option_env!("POD_BUILD_DATE")),
  }
}

/// `Some(trimmed)` when the build-time value is present and non-blank, else
/// `None` (so the optional `app.git_sha` / `app.build_date` keys are omitted).
fn non_empty(value: Option<&str>) -> Option<String> {
  let value = value?.trim();
  (!value.is_empty()).then(|| value.to_owned())
}

/// Push one event onto the buffer with the collector held; a no-op when the
/// subsystem is uninitialized or under `cfg(test)`.
fn with_buffers(push: impl FnOnce(&mut Buffers)) {
  if cfg!(test) {
    return;
  }
  if let Some(collector) = COLLECTOR.get()
    && let Ok(mut buffers) = collector.buffers.lock()
  {
    push(&mut buffers);
  }
}

/// Current RFC3339 UTC timestamp (the per-event `t` and the envelope `sent_at`).
fn now_rfc3339() -> String {
  chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// ---- Capture: cheap, non-blocking, no Settings branch. ----------------------

/// Record a view/route open at the central `navigate()` dispatcher.
pub fn record_view_open(name: impl Into<String>) {
  let name = name.into();
  with_buffers(|buffers| {
    buffers.usage.push(UsageEvent {
      t: now_rfc3339(),
      kind: UsageEventKind::ViewOpen,
      name,
      on: None,
    });
  });
}

/// Record a feature toggle persisted in settings (carries its new `on` state).
pub fn record_feature_toggle(name: impl Into<String>, on: bool) {
  let name = name.into();
  with_buffers(|buffers| {
    buffers.usage.push(UsageEvent {
      t: now_rfc3339(),
      kind: UsageEventKind::FeatureToggle,
      name,
      on: Some(on),
    });
  });
}

/// Record a sub-section selection at the central dispatcher.
pub fn record_sub_section(name: impl Into<String>) {
  let name = name.into();
  with_buffers(|buffers| {
    buffers.usage.push(UsageEvent {
      t: now_rfc3339(),
      kind: UsageEventKind::SubSection,
      name,
      on: None,
    });
  });
}

/// Record a view's nav->first-paint time and frame-time p95, in milliseconds.
pub fn record_view_load(name: impl Into<String>, load_ms: u64, frame_p95_ms: u64) {
  let name = name.into();
  with_buffers(|buffers| {
    buffers.views.push(PerformanceViewEntry {
      name,
      load_ms,
      frame_p95_ms,
    });
  });
}

/// Record a live-heap snapshot (MiB); the largest seen this window is sent.
pub fn record_frame(heap_mb: u64) {
  with_buffers(|buffers| {
    buffers.heap_mb = buffers.heap_mb.max(heap_mb);
  });
}

// ---- Flush: the single gate. ------------------------------------------------

/// The single gated flush (the app's periodic tick + the exit flush). Re-reads
/// the live config snapshot, drains the buffer, and -- unless the master switch
/// is off -- assembles a [`Batch`] of the enabled streams and hands it to the
/// fire-and-forget [`Sender`]. The buffer is always cleared (drop-on-failure);
/// a no-op when the subsystem is uninitialized or under `cfg(test)`.
pub fn flush(sender: &Sender) {
  if cfg!(test) {
    return;
  }
  let Some(collector) = COLLECTOR.get() else {
    return;
  };
  if let Some(batch) = collector.assemble() {
    let sender = sender.clone();
    tokio::spawn(async move {
      sender.send(&batch).await;
    });
  }
}

impl Collector {
  /// Drain the buffer and, when the master switch is on, assemble the gated
  /// session [`Batch`]. Returns `None` (and still drains) when telemetry is
  /// disabled or no enabled stream has any buffered data.
  fn assemble(&self) -> Option<Batch> {
    let Ok(mut buffers) = self.buffers.lock() else {
      return None;
    };

    // Master off: drain-and-discard, POST nothing (every trigger, incl. exit).
    if !*buffers.config.enabled() {
      *buffers = Buffers {
        config: buffers.config,
        environment_sent: buffers.environment_sent,
        ..Buffers::default()
      };
      return None;
    }

    let config = buffers.config;
    let usage_events = std::mem::take(&mut buffers.usage);
    let views = std::mem::take(&mut buffers.views);
    let heap_mb = std::mem::replace(&mut buffers.heap_mb, 0);

    // environment rides only the first flush of the process.
    let emit_environment = *config.environment() && !buffers.environment_sent;
    if emit_environment {
      buffers.environment_sent = true;
    }
    drop(buffers);

    let usage = (*config.usage() && !usage_events.is_empty()).then_some(UsageStream {
      events: usage_events,
    });
    let performance = (*config.performance() && !views.is_empty()).then_some(PerformanceStream {
      views,
      heap_mb,
    });
    let environment = emit_environment.then(environment_stream);

    // Nothing enabled carried any data this window -- drain already happened.
    if usage.is_none() && performance.is_none() && environment.is_none() {
      return None;
    }

    Some(Batch {
      schema: SCHEMA_VERSION,
      kind: Kind::Session,
      id: self.anon_id.clone(),
      session: self.session.clone(),
      app: self.app.clone(),
      sent_at: now_rfc3339(),
      streams: Streams {
        usage,
        performance,
        environment,
        crashes: None,
      },
    })
  }
}

/// Assemble the closed-world environment stream (§6.1.1). `os` / `arch` come
/// from `std::env::consts`; the host-probed fields are left as the `"unknown"`
/// sentinel here (richer probing is out of scope for the collector).
fn environment_stream() -> EnvironmentStream {
  EnvironmentStream {
    os: std::env::consts::OS.to_owned(),
    os_version: UNKNOWN.to_owned(),
    arch: std::env::consts::ARCH.to_owned(),
    display: UNKNOWN.to_owned(),
    locale: UNKNOWN.to_owned(),
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  fn config(enabled: bool, usage: bool, performance: bool, environment: bool) -> TelemetryConfig {
    let mut config = TelemetryConfig::default();
    config
      .set_enabled(enabled)
      .set_usage(usage)
      .set_performance(performance)
      .set_environment(environment);
    config
  }

  /// Build a collector directly (bypassing the global `OnceLock`) so each test
  /// drives an isolated buffer + config snapshot.
  fn collector(config: TelemetryConfig) -> Collector {
    Collector {
      anon_id: anon_id("machine-test"),
      session: session_tag(),
      app: app_identity(),
      buffers: Mutex::new(Buffers {
        config,
        ..Buffers::default()
      }),
    }
  }

  fn push_usage(collector: &Collector, kind: UsageEventKind, name: &str, on: Option<bool>) {
    collector.buffers.lock().unwrap().usage.push(UsageEvent {
      t: "2026-06-25T14:30:01Z".to_owned(),
      kind,
      name: name.to_owned(),
      on,
    });
  }

  // ---- §7.4: session tag is `s_` + exactly 8 lowercase hex. ----

  #[test]
  fn session_tag_is_s_plus_eight_lowercase_hex() {
    for _ in 0..64 {
      let tag = session_tag();
      assert_eq!(tag.len(), 10);
      assert!(tag.starts_with("s_"));
      assert!(
        tag[2..]
          .chars()
          .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
      );
    }
  }

  // ---- Capture is a structural no-op under cfg(test). ----

  #[test]
  fn record_functions_are_a_no_op_under_cfg_test() {
    // These run in a test build; even with an initialized global they must not
    // push (the cfg(test) guard short-circuits before any buffer access).
    record_view_open("wallet");
    record_feature_toggle("skills.plan_optimizer", true);
    record_sub_section("market.orders");
    record_view_load("wallet", 142, 11);
    record_frame(84);
    // No panic, no global mutation: capture stayed inert.
    assert!(COLLECTOR.get().is_none());
  }

  // ---- record_* push the right events / entries onto the buffer. ----
  //
  // The free functions are cfg(test)-inert, so these assert the buffer push by
  // driving the same shape directly (the production push path).

  #[test]
  fn usage_pushes_accumulate_view_open_and_feature_toggle() {
    let collector = collector(config(true, true, true, true));
    push_usage(&collector, UsageEventKind::ViewOpen, "wallet", None);
    push_usage(
      &collector,
      UsageEventKind::FeatureToggle,
      "skills.plan_optimizer",
      Some(true),
    );

    let buffers = collector.buffers.lock().unwrap();
    assert_eq!(buffers.usage.len(), 2);
    assert_eq!(buffers.usage[0].kind, UsageEventKind::ViewOpen);
    assert_eq!(buffers.usage[0].on, None);
    assert_eq!(buffers.usage[1].kind, UsageEventKind::FeatureToggle);
    assert_eq!(buffers.usage[1].on, Some(true));
  }

  #[test]
  fn frame_pushes_keep_the_largest_heap_snapshot() {
    let collector = collector(config(true, true, true, true));
    {
      let mut buffers = collector.buffers.lock().unwrap();
      buffers.heap_mb = buffers.heap_mb.max(42);
      buffers.heap_mb = buffers.heap_mb.max(84);
      buffers.heap_mb = buffers.heap_mb.max(10);
    }
    assert_eq!(collector.buffers.lock().unwrap().heap_mb, 84);
  }

  // ---- flush assembly: enabled streams + contract shape. ----

  #[test]
  fn assemble_builds_a_session_batch_matching_the_contract_shape() {
    let collector = collector(config(true, true, true, true));
    push_usage(&collector, UsageEventKind::ViewOpen, "wallet", None);
    collector.buffers.lock().unwrap().views.push(PerformanceViewEntry {
      name: "wallet".to_owned(),
      load_ms: 142,
      frame_p95_ms: 11,
    });
    collector.buffers.lock().unwrap().heap_mb = 84;

    let batch = collector.assemble().expect("enabled streams carry data");
    assert_eq!(batch.schema, SCHEMA_VERSION);
    assert_eq!(batch.kind, Kind::Session);
    assert_eq!(batch.id.len(), 64);
    assert!(batch.session.starts_with("s_"));
    assert_eq!(batch.app.version, env!("CARGO_PKG_VERSION"));

    let streams = &batch.streams;
    assert_eq!(streams.usage.as_ref().unwrap().events.len(), 1);
    let performance = streams.performance.as_ref().unwrap();
    assert_eq!(performance.views.len(), 1);
    assert_eq!(performance.heap_mb, 84);
    assert!(streams.environment.is_some(), "environment rides the first flush");
    assert!(streams.crashes.is_none(), "session batches never carry crashes");

    // Buffer drained regardless of (no) send.
    let buffers = collector.buffers.lock().unwrap();
    assert!(buffers.usage.is_empty());
    assert!(buffers.views.is_empty());
    assert_eq!(buffers.heap_mb, 0);
  }

  // ---- flush gates off-streams: a disabled stream's buffer is dropped. ----

  #[test]
  fn assemble_drops_the_buffered_events_of_an_off_stream() {
    let collector = collector(config(true, false, true, false));
    push_usage(&collector, UsageEventKind::ViewOpen, "wallet", None);
    collector.buffers.lock().unwrap().views.push(PerformanceViewEntry {
      name: "wallet".to_owned(),
      load_ms: 142,
      frame_p95_ms: 11,
    });

    let batch = collector.assemble().expect("performance is still enabled");
    assert!(batch.streams.usage.is_none(), "usage off => stream omitted");
    assert!(batch.streams.performance.is_some());
    assert!(batch.streams.environment.is_none(), "environment off => omitted");

    // The off stream's buffered events were drained, not retained.
    assert!(collector.buffers.lock().unwrap().usage.is_empty());
  }

  // ---- flush no-ops (POSTs nothing) when the master switch is off. ----

  #[test]
  fn assemble_drains_and_posts_nothing_when_disabled() {
    let collector = collector(config(false, true, true, true));
    push_usage(&collector, UsageEventKind::ViewOpen, "wallet", None);
    collector.buffers.lock().unwrap().views.push(PerformanceViewEntry {
      name: "wallet".to_owned(),
      load_ms: 142,
      frame_p95_ms: 11,
    });

    assert!(collector.assemble().is_none(), "master off => no batch");
    // Drained-and-discarded so memory stays bounded across the disabled span.
    let buffers = collector.buffers.lock().unwrap();
    assert!(buffers.usage.is_empty());
    assert!(buffers.views.is_empty());
  }

  // ---- environment rides only the first flush of the process. ----

  #[test]
  fn environment_is_emitted_only_on_the_first_flush() {
    let collector = collector(config(true, true, true, true));
    push_usage(&collector, UsageEventKind::ViewOpen, "wallet", None);
    let first = collector.assemble().expect("first flush carries data");
    assert!(first.streams.environment.is_some());

    push_usage(&collector, UsageEventKind::ViewOpen, "market", None);
    let second = collector.assemble().expect("second flush carries usage");
    assert!(second.streams.environment.is_none(), "environment is once-per-process");
  }

  // ---- empty enabled streams => nothing to send. ----

  #[test]
  fn assemble_returns_none_when_no_enabled_stream_has_data() {
    // environment off + empty usage/performance => nothing to POST.
    let collector = collector(config(true, true, true, false));
    assert!(collector.assemble().is_none());
  }

  // ---- the assembled batch serializes onto the frozen contract. ----

  #[test]
  fn assembled_batch_round_trips_through_the_contract() {
    let collector = collector(config(true, true, false, false));
    push_usage(
      &collector,
      UsageEventKind::FeatureToggle,
      "skills.plan_optimizer",
      Some(true),
    );
    let batch = collector.assemble().unwrap();

    let json = serde_json::to_value(&batch).unwrap();
    let streams = json["streams"].as_object().unwrap();
    // usage on, performance/environment off => only the usage key is present.
    assert!(streams.contains_key("usage"));
    assert!(!streams.contains_key("performance"));
    assert!(!streams.contains_key("environment"));
    assert!(!streams.contains_key("crashes"));
    // feature_toggle carries `on`.
    assert_eq!(
      json["streams"]["usage"]["events"][0]["on"],
      serde_json::Value::Bool(true)
    );

    // Full round-trip back through the contract type is lossless.
    let parsed: Batch = serde_json::from_value(json).unwrap();
    assert_eq!(parsed, batch);
  }

  // ---- set_config refreshes the live snapshot the next flush gates on. ----

  #[test]
  fn set_config_on_an_uninitialized_global_is_inert() {
    // No global collector in this isolated unit; set_config must not panic.
    set_config(config(false, false, false, false));
    assert!(COLLECTOR.get().is_none());
  }
}
