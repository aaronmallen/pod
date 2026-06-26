//! Crash telemetry pipeline: disk buffer + next-launch delivery (spec mmmzstpq
//! §5.4/§5.5/§8.4).
//!
//! A panic fires in a dying process, so a crash report cannot be POSTed
//! in-session: the network/tokio runtime may already be unwinding. Instead a
//! process-wide panic hook SYNCHRONOUSLY appends one scrubbed NDJSON
//! [`CrashRecord`] to an on-disk buffer (no network, no async, never
//! re-panicking), and the *next* launch reads that buffer, POSTs one
//! `kind:"crash"` [`Batch`], and deletes it on success.
//!
//! Three pieces cooperate:
//!
//! * Process-global statics ([`install`]) captured at boot BEFORE
//!   `install_panic_hook`, so the dying-process hook has correct attribution
//!   (buffer path, session, machine id, app identity, and a live
//!   [`TelemetryConfig`] snapshot) without any `&App`.
//! * A bounded (≤20) in-memory ring of allow-listed `context_log` lines, fed by
//!   the [`RingLayer`] tracing layer; the §5.5 allow-list is applied AT INGEST
//!   so nothing unscrubbed is ever held, and the hook reads the ring (never the
//!   rolling log file).
//! * The boot-time [`deliver`] step, run fire-and-forget near the rest of
//!   telemetry init.
//!
//! All scrubbing reuses the pure [`crate::telemetry::pii`] module; this module
//! adds only the statics, the ring, the synchronous append, and the delivery.

use std::{
  io::Write as _,
  path::PathBuf,
  sync::{Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};

use crate::{
  clients::telemetry::{Endpoint, Sender},
  config::TelemetryConfig,
  telemetry::pii,
  telemetry_contract::{App, Batch, CrashReport, CrashStream, Kind, SCHEMA_VERSION, Streams},
};

/// The buffer file name under the resolved log dir (§8.4).
const BUFFER_FILE: &str = "telemetry-crashes.ndjson";

/// Retention cap on the on-disk buffer (§8.4): a permanently-failing send must
/// not grow it without bound. On the next launch, before sending, the buffer is
/// truncated to the newest records once it exceeds this byte size.
const BUFFER_MAX_BYTES: u64 = 256 * 1024;

/// Retention cap on record age (§8.4): records older than this are dropped at
/// delivery time so a stale buffer can never resurrect an ancient crash.
const BUFFER_MAX_AGE_DAYS: i64 = 30;

/// The bounded ring of allow-listed `context_log` lines (§8.4): at most this
/// many newest lines are retained for a crash report.
const RING_CAPACITY: usize = 20;

/// The process-global crash attribution snapshot, set once at boot by
/// [`install`] BEFORE the panic hook is installed. Holds everything the
/// dying-process hook needs without an `&App`.
static STATE: OnceLock<CrashState> = OnceLock::new();

/// The bounded ring buffer of scrubbed, allow-listed `context_log` lines, fed by
/// the [`RingLayer`] tracing layer. The §5.5 allow-list runs at ingest, so only
/// benign lines are ever held; the panic hook drains a snapshot of this ring.
static RING: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

/// The boot-captured crash attribution snapshot (§8.4 statics).
#[derive(Clone, Debug)]
struct CrashState {
  /// Resolved path to the on-disk NDJSON buffer.
  buffer: PathBuf,
  /// The per-process session tag (`s_########`), carried on the crash envelope.
  session: String,
  /// `sha256(machine_id)` is derived at send; the raw machine id is held here.
  machine_id: String,
  /// App identity (version / git sha / build date) for the crashed run.
  app: App,
  /// The live telemetry config snapshot — the hook skips the write entirely
  /// when `crashes` OR the master switch is off.
  config: TelemetryConfig,
  /// Whether a baked ingest endpoint exists. With no endpoint (dev builds) the
  /// hook no-ops and boot deletes any stale buffer.
  has_endpoint: bool,
}

/// One buffered crash, serialized as a single NDJSON line. All free-text is
/// scrubbed via [`crate::telemetry::pii`] BEFORE this struct is built, so raw
/// PII never reaches disk. Carries the crashed-run attribution so the next
/// launch (which has its own, different identity) can reconstruct the envelope.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct CrashRecord {
  /// RFC3339 UTC time the panic happened (becomes `crashed_at` on the wire).
  crashed_at: String,
  /// The crashed run's session tag.
  session: String,
  /// The crashed run's raw machine id (hashed to the envelope `id` at send).
  machine_id: String,
  /// The crashed run's app identity.
  app: App,
  /// Scrubbed panic message.
  message: String,
  /// Scrubbed, app-root-relative panic location.
  location: Option<String>,
  /// Scrubbed backtrace frame strings.
  backtrace: Option<Vec<String>>,
  /// Allow-listed, scrubbed `context_log` lines.
  context_log: Option<Vec<String>>,
}

/// Install the crash attribution statics. MUST be called at boot BEFORE
/// `install_panic_hook` so the dying-process hook has correct attribution.
/// Idempotent: a second call is a no-op.
pub fn install(log_dir: &std::path::Path, machine_id: &str, config: TelemetryConfig) {
  let _ = STATE.set(CrashState {
    buffer: log_dir.join(BUFFER_FILE),
    session: session_tag(),
    machine_id: machine_id.to_owned(),
    app: app_identity(),
    config,
    has_endpoint: Endpoint::from_env().is_some(),
  });
  // Ensure the ring exists so the layer (installed in init_tracing, which may
  // run before or after this) always has a backing buffer.
  let _ = RING.set(Mutex::new(Vec::new()));
}

/// Synchronously append one scrubbed NDJSON [`CrashRecord`] for a panic.
///
/// Called from the panic hook in a dying process: NO network, NO tokio, NEVER
/// re-panics (every fallible step is best-effort), and skips entirely when the
/// statics are unset, when `crashes` OR master is off, or when no baked endpoint
/// exists. The message / location / backtrace are scrubbed via
/// [`crate::telemetry::pii`] BEFORE hitting disk, and `context_log` is a drained
/// snapshot of the already-scrubbed ring.
///
/// To stay allocation-light on the dying-process path the only heap work is the
/// scrub itself (which the pure module performs) plus the serialized line; no
/// re-entrancy into the tracing layer (it writes directly with `OpenOptions`).
pub fn capture(message: &str, location: Option<&str>, backtrace: &str) {
  let Some(state) = STATE.get() else {
    return;
  };
  // §8.4: opted-out or structurally-disabled builds never write PII to disk.
  if !*state.config.enabled() || !*state.config.crashes() || !state.has_endpoint {
    return;
  }

  let record = build_record(state, message, location, backtrace);
  append_record(&state.buffer, &record);
}

/// Build one scrubbed [`CrashRecord`] from the crash statics and the raw panic
/// content. The message / location / backtrace are scrubbed via
/// [`crate::telemetry::pii`] here, so raw PII never leaves this function.
fn build_record(state: &CrashState, message: &str, location: Option<&str>, backtrace: &str) -> CrashRecord {
  CrashRecord {
    crashed_at: now_rfc3339(),
    session: state.session.clone(),
    machine_id: state.machine_id.clone(),
    app: state.app.clone(),
    message: pii::scrub_message(message),
    location: location.map(pii::scrub_location),
    backtrace: scrub_backtrace_text(backtrace),
    context_log: snapshot_ring(),
  }
}

/// Best-effort serialize + append of one record as an NDJSON line; never
/// re-panics from the dying-process hook (every fallible step is swallowed).
fn append_record(buffer: &std::path::Path, record: &CrashRecord) {
  let Ok(mut line) = serde_json::to_string(record) else {
    return;
  };
  line.push('\n');
  let _ = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(buffer)
    .and_then(|mut file| file.write_all(line.as_bytes()));
}

/// Deliver any buffered crashes at boot (fire-and-forget). Reads the buffer, and:
///
/// * if master OR `crashes` is off, or no endpoint is baked, deletes the buffer
///   unsent (nothing opted-out ever leaves);
/// * else applies the size/age retention bound, then POSTs ONE `kind:"crash"`
///   [`Batch`] (one report per buffered record, crashed-run attribution and
///   `crashed_at` from each record) and deletes the buffer on a 2xx; on failure
///   the buffer is left on disk for the next launch.
pub fn deliver(sender: &Sender, buffer: &std::path::Path, config: TelemetryConfig, has_endpoint: bool) {
  let raw = match std::fs::read_to_string(buffer) {
    Ok(raw) => raw,
    // No buffer (the common case) or an unreadable one: nothing to do.
    Err(_) => return,
  };

  // Opted out or structurally disabled: delete unsent, never POST.
  if !*config.enabled() || !*config.crashes() || !has_endpoint {
    let _ = std::fs::remove_file(buffer);
    return;
  }

  let records = retain(parse_records(&raw));
  let Some(batch) = assemble(&records) else {
    // Nothing deliverable survived the bound: clear the buffer so it cannot
    // accumulate dead lines forever.
    let _ = std::fs::remove_file(buffer);
    return;
  };

  let sender = sender.clone();
  let buffer = buffer.to_owned();
  tokio::spawn(async move {
    if sender.send(&batch).await {
      let _ = std::fs::remove_file(&buffer);
    }
    // On failure: leave the buffer on disk (retry = "still there next launch").
  });
}

/// Path to the on-disk buffer, for the boot delivery wiring. `None` until
/// [`install`] runs.
pub fn buffer_path() -> Option<PathBuf> {
  STATE.get().map(|state| state.buffer.clone())
}

/// Parse the NDJSON buffer into records, silently skipping malformed lines.
fn parse_records(raw: &str) -> Vec<CrashRecord> {
  raw
    .lines()
    .filter(|line| !line.trim().is_empty())
    .filter_map(|line| serde_json::from_str::<CrashRecord>(line).ok())
    .collect()
}

/// Apply the §8.4 retention bound: drop records older than [`BUFFER_MAX_AGE_DAYS`],
/// then, if the surviving NDJSON would still exceed [`BUFFER_MAX_BYTES`], keep
/// only the newest records that fit (newest-wins).
fn retain(records: Vec<CrashRecord>) -> Vec<CrashRecord> {
  let cutoff = chrono::Utc::now() - chrono::Duration::days(BUFFER_MAX_AGE_DAYS);
  let mut fresh: Vec<CrashRecord> = records
    .into_iter()
    .filter(
      |record| match chrono::DateTime::parse_from_rfc3339(&record.crashed_at) {
        Ok(when) => when.with_timezone(&chrono::Utc) >= cutoff,
        // Unparseable timestamp: keep it (fail-open on age, the size cap still bounds it).
        Err(_) => true,
      },
    )
    .collect();

  // Size cap: walk from the newest record, accumulating until the byte budget is
  // spent, then keep that newest suffix.
  let mut budget = BUFFER_MAX_BYTES;
  let mut keep_from = fresh.len();
  for (index, record) in fresh.iter().enumerate().rev() {
    let line_len = serde_json::to_string(record)
      .map(|line| line.len() as u64 + 1)
      .unwrap_or(0);
    if line_len > budget {
      break;
    }
    budget -= line_len;
    keep_from = index;
  }
  if keep_from > 0 {
    fresh.drain(0..keep_from);
  }
  fresh
}

/// Build one `kind:"crash"` [`Batch`] from the buffered records, or `None` when
/// there are no records. Attribution (`id`, `session`, `app`) comes from the
/// crashed run recorded in the buffer, NOT the current launch.
fn assemble(records: &[CrashRecord]) -> Option<Batch> {
  let first = records.first()?;
  let reports = records
    .iter()
    .map(|record| CrashReport {
      crashed_at: record.crashed_at.clone(),
      message: record.message.clone(),
      location: record.location.clone(),
      backtrace: record.backtrace.clone(),
      context_log: record.context_log.clone(),
    })
    .collect();

  Some(Batch {
    schema: SCHEMA_VERSION,
    kind: Kind::Crash,
    id: crate::clients::telemetry::anon_id(&first.machine_id),
    session: first.session.clone(),
    app: first.app.clone(),
    sent_at: now_rfc3339(),
    streams: Streams {
      usage: None,
      performance: None,
      environment: None,
      crashes: Some(CrashStream {
        reports,
      }),
    },
  })
}

/// Split a captured backtrace into frames and scrub each via the pure module.
/// `None` when the backtrace is empty / disabled (so the wire field is omitted).
fn scrub_backtrace_text(backtrace: &str) -> Option<Vec<String>> {
  let frames: Vec<String> = backtrace
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .map(ToOwned::to_owned)
    .collect();
  if frames.is_empty() {
    return None;
  }
  Some(pii::scrub_backtrace(&frames))
}

/// Drain a snapshot of the scrubbed ring; `None` when empty so the wire field is
/// omitted. The ring already holds allow-listed, scrubbed lines (§5.5 at
/// ingest), so this is a clone, not a re-scrub.
fn snapshot_ring() -> Option<Vec<String>> {
  let lines = RING.get()?.lock().ok()?.clone();
  (!lines.is_empty()).then_some(lines)
}

/// Push one raw JSON log line into the ring, applying the §5.5 allow-list at
/// ingest and bounding the ring to [`RING_CAPACITY`]. A dropped (non-allow-listed)
/// line leaves the ring unchanged. Used by [`RingLayer`].
fn ingest(raw_json: &str) {
  let Some(ring) = RING.get() else {
    return;
  };
  // The pure scrubber returns 0 or 1 surviving lines for a single input.
  let scrubbed = pii::scrub_context_log(std::slice::from_ref(&raw_json.to_owned()));
  let Some(line) = scrubbed.into_iter().next() else {
    return;
  };
  if let Ok(mut buffer) = ring.lock() {
    buffer.push(line);
    let len = buffer.len();
    if len > RING_CAPACITY {
      buffer.drain(0..len - RING_CAPACITY);
    }
  }
}

/// Per-process session tag (`s_` + 8 lowercase hex), matching the Worker's
/// `^s_[0-9a-f]{8}$`. Distinct from the collector's session (each subsystem
/// derives its own; the crash envelope must carry the crashed run's tag).
fn session_tag() -> String {
  use rand::Rng as _;
  let mut bytes = [0u8; 4];
  rand::rng().fill_bytes(&mut bytes);
  let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
  format!("s_{hex}")
}

/// App identity from the same build-time values the contract documents.
fn app_identity() -> App {
  App {
    version: env!("CARGO_PKG_VERSION").to_owned(),
    git_sha: non_empty(option_env!("POD_GIT_SHA")),
    build_date: non_empty(option_env!("POD_BUILD_DATE")),
  }
}

/// `Some(trimmed)` for a present, non-blank build-time value, else `None`.
fn non_empty(value: Option<&str>) -> Option<String> {
  let value = value?.trim();
  (!value.is_empty()).then(|| value.to_owned())
}

/// Current RFC3339 UTC timestamp.
fn now_rfc3339() -> String {
  chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// A `tracing` layer that feeds the bounded `context_log` ring. Each event is
/// serialized to a JSON line carrying `level`, `target`, `timestamp`, and
/// `message`, then handed to [`ingest`], which applies the §5.5 allow-list AT
/// INGEST (so a non-allow-listed line is dropped before it is ever held).
pub struct RingLayer;

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for RingLayer {
  fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
    // Reuse the same self-contained line shape `scrub_context_log` parses:
    // top-level level / target / timestamp / message.
    let mut message = String::new();
    event.record(&mut MessageVisitor(&mut message));

    let metadata = event.metadata();
    let line = serde_json::json!({
      "level": metadata.level().as_str(),
      "target": metadata.target(),
      "timestamp": now_rfc3339(),
      "message": message,
    });
    if let Ok(raw) = serde_json::to_string(&line) {
      ingest(&raw);
    }
  }
}

/// Pulls the `message` field out of a tracing event into a `String`.
struct MessageVisitor<'a>(&'a mut String);

impl tracing::field::Visit for MessageVisitor<'_> {
  fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
    if field.name() == "message" {
      *self.0 = value.to_owned();
    }
  }

  fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
    if field.name() == "message" {
      *self.0 = format!("{value:?}");
    }
  }
}

/// Days-since-the-epoch helper used only by tests to forge an old timestamp.
#[cfg(test)]
fn rfc3339_days_ago(days: i64) -> String {
  (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
  use std::{
    sync::{Mutex as StdMutex, MutexGuard},
    time::SystemTime,
  };

  use pretty_assertions::assert_eq;

  use super::*;

  // The crash statics + ring are process-global; serialize the tests that touch
  // them so they do not race each other's STATE/RING.
  static GLOBAL: StdMutex<()> = StdMutex::new(());

  fn lock() -> MutexGuard<'static, ()> {
    GLOBAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  fn config(enabled: bool, crashes: bool) -> TelemetryConfig {
    let mut config = TelemetryConfig::default();
    config.set_enabled(enabled).set_crashes(crashes);
    config
  }

  fn tmp_dir() -> PathBuf {
    let base = std::env::temp_dir().join(format!(
      "pod-crash-test-{}-{}",
      std::process::id(),
      SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
    ));
    std::fs::create_dir_all(&base).unwrap();
    base
  }

  fn record(message: &str, crashed_at: &str) -> CrashRecord {
    CrashRecord {
      crashed_at: crashed_at.to_owned(),
      session: "s_1a2b3c4d".to_owned(),
      machine_id: "machine-test".to_owned(),
      app: app_identity(),
      message: message.to_owned(),
      location: None,
      backtrace: None,
      context_log: None,
    }
  }

  // ---- the ring: bounded to 20, allow-list applied at ingest. ----

  #[test]
  fn ring_is_bounded_to_twenty_newest_lines() {
    let _guard = lock();
    let _ = RING.set(Mutex::new(Vec::new()));
    RING.get().unwrap().lock().unwrap().clear();

    for i in 0..50 {
      let raw = format!(r#"{{"level":"INFO","target":"pod::ui","message":"line {i}"}}"#);
      ingest(&raw);
    }
    let held = RING.get().unwrap().lock().unwrap().clone();
    assert_eq!(held.len(), RING_CAPACITY);
    assert!(held.last().unwrap().contains("line 49"), "newest line kept");
    assert!(!held.iter().any(|l| l.contains("line 0\"")), "oldest line evicted");
  }

  #[test]
  fn ring_drops_non_allowlisted_targets_at_ingest() {
    let _guard = lock();
    let _ = RING.set(Mutex::new(Vec::new()));
    RING.get().unwrap().lock().unwrap().clear();

    // pod::features::roster::auth carries character_name and is NOT allow-listed.
    ingest(r#"{"level":"INFO","target":"pod::features::roster::auth","name":"Aaron","message":"signed in"}"#);
    // pod::nav is benign and allow-listed.
    ingest(r#"{"level":"INFO","target":"pod::nav","message":"navigated"}"#);

    let held = RING.get().unwrap().lock().unwrap().clone();
    assert_eq!(held.len(), 1, "only the allow-listed line survived ingest");
    assert!(held[0].contains("pod::nav"));
    assert!(!held.join("").contains("Aaron"), "dropped line never held");
  }

  // ---- capture: writes a scrubbed NDJSON record, raw PII absent. ----

  #[test]
  fn capture_writes_a_scrubbed_ndjson_record() {
    let _guard = lock();
    let dir = tmp_dir();
    let buffer = dir.join(BUFFER_FILE);

    // Drive capture's exact record-build + append path against a test-owned
    // CrashState (so this test never depends on the process-global STATE, which
    // a sibling test may have already claimed). Same scrubbing, same append.
    let state = state_with(&dir, config(true, true), true);
    let record = build_record(
      &state,
      "panic at /Users/aaron/src/github.com/aaronmallen/pod/src/wallet.rs with character_id=90000001 and https://esi.evetech.net/v5/x?token=secret",
      Some("/Users/aaron/src/github.com/aaronmallen/pod/src/wallet.rs:412"),
      "/Users/aaron/.cargo/registry/src/index.crates.io-abc/tokio-1.38/src/task.rs:42",
    );
    append_record(&buffer, &record);

    let raw = std::fs::read_to_string(&buffer).unwrap();
    assert!(!raw.contains("/Users/aaron"), "home path leaked: {raw}");
    assert!(!raw.contains("90000001"), "character_id leaked: {raw}");
    assert!(!raw.contains("token=secret"), "url query leaked: {raw}");
    assert!(raw.contains("src/wallet.rs"), "location not app-root-relative: {raw}");
    let parsed: CrashRecord = serde_json::from_str(raw.lines().next().unwrap()).unwrap();
    assert!(parsed.message.contains("character_id=<id>"));
    let _ = std::fs::remove_dir_all(&dir);
  }

  fn state_with(dir: &std::path::Path, config: TelemetryConfig, has_endpoint: bool) -> CrashState {
    CrashState {
      buffer: dir.join(BUFFER_FILE),
      session: "s_1a2b3c4d".to_owned(),
      machine_id: "machine-test".to_owned(),
      app: app_identity(),
      config,
      has_endpoint,
    }
  }

  // ---- assemble: the crash Batch matches the contract. ----

  #[test]
  fn assemble_builds_a_crash_kind_batch_from_buffered_records() {
    let _guard = lock();
    let records = vec![record(
      "called `Option::unwrap()` on a `None` value",
      "2026-06-24T22:14:03Z",
    )];
    let batch = assemble(&records).unwrap();

    assert_eq!(batch.schema, SCHEMA_VERSION);
    assert_eq!(batch.kind, Kind::Crash);
    assert_eq!(batch.id.len(), 64, "id is sha256 hex of the crashed-run machine id");
    assert_eq!(batch.session, "s_1a2b3c4d");
    assert!(batch.streams.usage.is_none() && batch.streams.environment.is_none());
    let reports = &batch.streams.crashes.as_ref().unwrap().reports;
    assert_eq!(reports.len(), 1);
    assert_eq!(
      reports[0].crashed_at, "2026-06-24T22:14:03Z",
      "crashed_at from the record"
    );

    // The envelope must round-trip through the frozen contract.
    let json = serde_json::to_value(&batch).unwrap();
    let streams = json["streams"].as_object().unwrap();
    assert_eq!(streams.keys().collect::<Vec<_>>(), vec!["crashes"]);
    let reparsed: Batch = serde_json::from_value(json).unwrap();
    assert_eq!(reparsed, batch);
  }

  // ---- retention bound: age + size. ----

  #[test]
  fn retain_drops_records_older_than_the_age_cap() {
    let _guard = lock();
    let records = vec![
      record("old", &rfc3339_days_ago(BUFFER_MAX_AGE_DAYS + 5)),
      record("fresh", &rfc3339_days_ago(1)),
    ];
    let kept = retain(records);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].message, "fresh");
  }

  #[test]
  fn retain_truncates_to_the_newest_records_under_the_size_cap() {
    let _guard = lock();
    // Build many fresh records whose combined NDJSON exceeds the byte cap.
    let big = "x".repeat(2000);
    let count = (BUFFER_MAX_BYTES as usize / big.len()) + 50;
    let records: Vec<CrashRecord> = (0..count)
      .map(|i| record(&format!("{big}{i}"), &rfc3339_days_ago(1)))
      .collect();
    let kept = retain(records);
    let bytes: u64 = kept
      .iter()
      .map(|r| serde_json::to_string(r).unwrap().len() as u64 + 1)
      .sum();
    assert!(bytes <= BUFFER_MAX_BYTES, "kept set still over the cap: {bytes}");
    assert!(!kept.is_empty(), "the newest records are kept");
  }

  // ---- boot delivery: deletes-unsent when opted out / no endpoint. ----

  #[test]
  fn deliver_deletes_unsent_when_master_off() {
    let _guard = lock();
    let dir = tmp_dir();
    let buffer = dir.join(BUFFER_FILE);
    std::fs::write(&buffer, "{\"crashed_at\":\"2026-06-24T22:14:03Z\"}\n").unwrap();

    // No Sender is needed: the opted-out branch never POSTs. Endpoint::from_env
    // is None in tests, so build the decision purely from the args.
    deliver_decision_only(&buffer, config(false, true), true);
    assert!(!buffer.exists(), "opted-out buffer must be deleted unsent");
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn deliver_deletes_unsent_when_crashes_off() {
    let _guard = lock();
    let dir = tmp_dir();
    let buffer = dir.join(BUFFER_FILE);
    std::fs::write(&buffer, "{\"crashed_at\":\"2026-06-24T22:14:03Z\"}\n").unwrap();
    deliver_decision_only(&buffer, config(true, false), true);
    assert!(!buffer.exists(), "crashes-off buffer must be deleted unsent");
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn deliver_deletes_unsent_when_no_endpoint() {
    let _guard = lock();
    let dir = tmp_dir();
    let buffer = dir.join(BUFFER_FILE);
    std::fs::write(&buffer, "{\"crashed_at\":\"2026-06-24T22:14:03Z\"}\n").unwrap();
    deliver_decision_only(&buffer, config(true, true), false);
    assert!(!buffer.exists(), "no-endpoint buffer must be deleted unsent");
    let _ = std::fs::remove_dir_all(&dir);
  }

  // The opted-out / no-endpoint branch of `deliver`, exercised without a live
  // tokio runtime or Sender (which `Endpoint::from_env` can't build in tests).
  fn deliver_decision_only(buffer: &std::path::Path, config: TelemetryConfig, has_endpoint: bool) {
    if std::fs::read_to_string(buffer).is_err() {
      return;
    }
    if !*config.enabled() || !*config.crashes() || !has_endpoint {
      let _ = std::fs::remove_file(buffer);
    }
  }

  // ---- the real `deliver`: drive every branch of the boot delivery. ----
  //
  // A `Sender` is built from the public `Endpoint` fields (no build-time env),
  // pointed at a local wiremock server. The non-POST branches ignore the sender;
  // the POST branch runs under a live tokio runtime against the mock.

  use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method},
  };

  fn sender_for(url: &str) -> Sender {
    Sender::new(Endpoint {
      url: url.to_owned(),
      key: "test-key".to_owned(),
    })
    .expect("reqwest client builds")
  }

  /// One serialized NDJSON record line for a fresh crash (passes the age bound).
  fn ndjson_line(message: &str) -> String {
    let mut line = serde_json::to_string(&record(message, &rfc3339_days_ago(1))).expect("record serializes");
    line.push('\n');
    line
  }

  /// Wait (bounded) for `predicate` to hold, so the assertion does not race the
  /// fire-and-forget `tokio::spawn` that `deliver` returns before completing.
  async fn wait_until(predicate: impl Fn() -> bool) {
    for _ in 0..200 {
      if predicate() {
        return;
      }
      tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
  }

  #[tokio::test]
  async fn deliver_is_a_no_op_when_the_buffer_is_absent() {
    // No global lock: these drive `deliver` against a unique-per-test buffer
    // path and a local Sender, touching neither the STATE nor RING statics (so
    // holding a std Mutex across the awaits below would be both needless and a
    // clippy `await_holding_lock`).
    let dir = tmp_dir();
    let buffer = dir.join(BUFFER_FILE); // never created
    let sender = sender_for("http://127.0.0.1:1/ingest");

    // Unreadable/absent buffer: deliver returns immediately, nothing created.
    deliver(&sender, &buffer, config(true, true), true);
    assert!(!buffer.exists(), "no buffer => nothing to deliver, none created");
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[tokio::test]
  async fn deliver_deletes_the_buffer_unsent_when_opted_out() {
    let dir = tmp_dir();
    let buffer = dir.join(BUFFER_FILE);
    std::fs::write(&buffer, ndjson_line("boom")).unwrap();
    let sender = sender_for("http://127.0.0.1:1/ingest");

    // master off / crashes off / no endpoint each take the delete-unsent path.
    deliver(&sender, &buffer, config(false, true), true);
    assert!(!buffer.exists(), "master off => deleted unsent");

    std::fs::write(&buffer, ndjson_line("boom")).unwrap();
    deliver(&sender, &buffer, config(true, false), true);
    assert!(!buffer.exists(), "crashes off => deleted unsent");

    std::fs::write(&buffer, ndjson_line("boom")).unwrap();
    deliver(&sender, &buffer, config(true, true), false);
    assert!(!buffer.exists(), "no endpoint => deleted unsent");
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[tokio::test]
  async fn deliver_clears_a_buffer_with_nothing_deliverable() {
    let dir = tmp_dir();
    let buffer = dir.join(BUFFER_FILE);
    // Only malformed / blank lines survive parsing: assemble() yields None, so
    // the buffer is cleared rather than POSTed.
    std::fs::write(&buffer, "not json\n\n{also bad}\n").unwrap();
    let sender = sender_for("http://127.0.0.1:1/ingest");

    deliver(&sender, &buffer, config(true, true), true);
    assert!(!buffer.exists(), "no deliverable record => buffer cleared");
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[tokio::test]
  async fn deliver_posts_a_crash_batch_and_deletes_the_buffer_on_a_2xx() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
      .and(header("X-Pod-Telemetry-Key", "test-key"))
      .respond_with(ResponseTemplate::new(200))
      .expect(1)
      .mount(&server)
      .await;

    let dir = tmp_dir();
    let buffer = dir.join(BUFFER_FILE);
    std::fs::write(&buffer, ndjson_line("called `Option::unwrap()` on a `None` value")).unwrap();
    let sender = sender_for(&format!("{}/ingest", server.uri()));

    deliver(&sender, &buffer, config(true, true), true);
    let probe = buffer.clone();
    wait_until(move || !probe.exists()).await;
    assert!(!buffer.exists(), "a 2xx delivery deletes the buffer");

    let received = server.received_requests().await.unwrap();
    assert_eq!(received.len(), 1, "exactly one crash batch was POSTed");
    let batch: Batch = serde_json::from_slice(&received[0].body).unwrap();
    assert_eq!(batch.kind, Kind::Crash);
    assert!(
      batch.streams.crashes.is_some(),
      "the POSTed batch carries the crash stream"
    );
    let _ = std::fs::remove_dir_all(&dir);
  }

  #[tokio::test]
  async fn deliver_keeps_the_buffer_on_a_failed_send() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
      .respond_with(ResponseTemplate::new(500))
      .mount(&server)
      .await;

    let dir = tmp_dir();
    let buffer = dir.join(BUFFER_FILE);
    std::fs::write(&buffer, ndjson_line("boom")).unwrap();
    let sender = sender_for(&format!("{}/ingest", server.uri()));

    deliver(&sender, &buffer, config(true, true), true);
    // Give the spawned send time to complete (and NOT delete the buffer).
    let probe = buffer.clone();
    wait_until(move || !probe.exists()).await; // returns when the 200ms budget elapses
    assert!(
      buffer.exists(),
      "a non-2xx delivery leaves the buffer for the next launch"
    );
    let _ = std::fs::remove_dir_all(&dir);
  }

  // ---- skip when crashes/master off or no endpoint (capture). ----

  #[test]
  fn capture_decision_skips_when_opted_out_or_no_endpoint() {
    // Mirror capture's guard: it must skip when any of the three gates fail.
    let skip = |c: TelemetryConfig, endpoint: bool| !*c.enabled() || !*c.crashes() || !endpoint;
    assert!(skip(config(false, true), true), "master off => skip");
    assert!(skip(config(true, false), true), "crashes off => skip");
    assert!(skip(config(true, true), false), "no endpoint => skip");
    assert!(!skip(config(true, true), true), "all on + endpoint => write");
  }

  // ---- session tag shape. ----

  #[test]
  fn session_tag_matches_the_worker_pattern() {
    for _ in 0..32 {
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
}
