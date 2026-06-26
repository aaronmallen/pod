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
  /// The most recent primary-window logical size (`"WxH"`), captured via
  /// [`set_display`]. `None` until the main window is sized; the environment
  /// stream then falls back to the `"unknown"` sentinel.
  display: Option<String>,
  /// The in-flight nav->first-paint timer: the route token opened at the last
  /// [`record_view_open`] plus the [`Instant`] it started. Closed by
  /// [`record_view_loaded`] when that route first paints (§8.3 `load_ms`).
  nav_started: Option<(String, std::time::Instant)>,
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

/// Record the primary-window logical size so the once-per-process `environment`
/// stream can report `display` as `"WxH"` (§8.2). Called from the main-window
/// geometry path; a no-op when the subsystem is uninitialized or under
/// `cfg(test)`. The latest size seen before the first flush wins.
pub fn set_display(width: u32, height: u32) {
  with_buffers(|buffers| {
    buffers.display = Some(format!("{width}x{height}"));
  });
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

/// Record a view/route open at the central `navigate()` dispatcher. Also arms
/// the nav->first-paint timer for this route (§8.3 `load_ms`), closed by the
/// first [`record_view_loaded`] of the same route.
pub fn record_view_open(name: impl Into<String>) {
  let name = name.into();
  with_buffers(|buffers| {
    buffers.nav_started = Some((name.clone(), std::time::Instant::now()));
    buffers.usage.push(UsageEvent {
      t: now_rfc3339(),
      kind: UsageEventKind::ViewOpen,
      name,
      on: None,
    });
  });
}

/// Close the nav->first-paint timer for `name` when its route first paints,
/// pushing a performance view entry with the elapsed `load_ms` (§8.3). A no-op
/// when no timer is armed or the painting route differs from the one opened
/// (so only the first paint of the just-navigated route is timed). `frame_p95`
/// is left at 0 here (the redraw-delta approximation is a deferred follow-up).
pub fn record_view_loaded(name: impl Into<String>) {
  let name = name.into();
  with_buffers(|buffers| {
    let Some((opened, started)) = buffers.nav_started.as_ref() else {
      return;
    };
    if *opened != name {
      return;
    }
    let load_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    buffers.nav_started = None;
    buffers.views.push(PerformanceViewEntry {
      name,
      load_ms,
      frame_p95_ms: 0,
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
    let display = buffers.display.clone();
    drop(buffers);

    let usage = (*config.usage() && !usage_events.is_empty()).then_some(UsageStream {
      events: usage_events,
    });
    let performance = (*config.performance() && !views.is_empty()).then_some(PerformanceStream {
      views,
      heap_mb,
    });
    let environment = emit_environment.then(|| environment_stream(display.as_deref()));

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

/// Assemble the closed-world environment stream (§6.1.1, §8.2). `os` / `arch`
/// come from `std::env::consts`; `os_version` (major only) and `locale`
/// (language only) are probed cheaply per-platform; `display` is the
/// primary-window logical size captured via [`set_display`]. Every
/// unresolvable field collapses to the literal `"unknown"` sentinel (never
/// omitted).
fn environment_stream(display: Option<&str>) -> EnvironmentStream {
  EnvironmentStream {
    os: std::env::consts::OS.to_owned(),
    os_version: os_version_major().unwrap_or_else(|| UNKNOWN.to_owned()),
    arch: std::env::consts::ARCH.to_owned(),
    display: display.map(str::to_owned).unwrap_or_else(|| UNKNOWN.to_owned()),
    locale: locale_language().unwrap_or_else(|| UNKNOWN.to_owned()),
  }
}

/// The major OS version, probed per-platform with one cheap subprocess /
/// file read (runs once, on the first flush). `None` when unresolvable, so the
/// caller substitutes the `"unknown"` sentinel.
///
/// macOS: `sw_vers -productVersion` (e.g. `15.5` -> `15`).
/// Linux: `/etc/os-release` `VERSION_ID` (e.g. `22.04` -> `22`).
/// Windows: `cmd /c ver` (e.g. `... [Version 10.0.19045.0]` -> `10`).
fn os_version_major() -> Option<String> {
  let raw = if cfg!(target_os = "macos") {
    std::process::Command::new("sw_vers")
      .arg("-productVersion")
      .output()
      .ok()
      .filter(|out| out.status.success())
      .and_then(|out| parse_sw_vers(&String::from_utf8_lossy(&out.stdout)))
  } else if cfg!(target_os = "windows") {
    std::process::Command::new("cmd")
      .args(["/c", "ver"])
      .output()
      .ok()
      .filter(|out| out.status.success())
      .and_then(|out| parse_windows_ver(&String::from_utf8_lossy(&out.stdout)))
  } else {
    std::fs::read_to_string("/etc/os-release")
      .ok()
      .and_then(|contents| parse_os_release_version_id(&contents))
  }?;

  major_component(&raw)
}

/// The raw `sw_vers -productVersion` line, trimmed (`"15.5\n"` -> `"15.5"`),
/// `None` when the output is blank. The major reduction is the caller's.
fn parse_sw_vers(stdout: &str) -> Option<String> {
  let trimmed = stdout.trim();
  (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// The version token out of `cmd /c ver` output
/// (`"... [Version 10.0.19045.0]"` -> `"10.0.19045.0]"`), `None` when the
/// `"Version "` marker is absent. The major reduction is the caller's.
fn parse_windows_ver(stdout: &str) -> Option<String> {
  stdout.split_once("Version ").map(|(_, rest)| rest.trim().to_owned())
}

/// The unquoted `VERSION_ID` value out of `/etc/os-release` contents
/// (`VERSION_ID="22.04"` -> `"22.04"`), `None` when the key is absent. The
/// major reduction is the caller's.
fn parse_os_release_version_id(contents: &str) -> Option<String> {
  contents.lines().find_map(|line| {
    line
      .strip_prefix("VERSION_ID=")
      .map(|value| value.trim_matches('"').to_owned())
  })
}

/// The leading numeric component of a dotted version string (`"15.5"` -> `"15"`),
/// `None` when there is no leading digit run.
fn major_component(version: &str) -> Option<String> {
  let major: String = version.trim().chars().take_while(char::is_ascii_digit).collect();
  (!major.is_empty()).then_some(major)
}

/// The language-only locale (region subtag dropped, §5.3): the first of
/// `LC_ALL` / `LC_MESSAGES` / `LANG` that is set and non-empty, reduced to its
/// language prefix (`"en_ZA.UTF-8"` -> `"en"`). `None` when none are set, so the
/// caller substitutes the `"unknown"` sentinel.
fn locale_language() -> Option<String> {
  ["LC_ALL", "LC_MESSAGES", "LANG"]
    .into_iter()
    .find_map(|key| std::env::var(key).ok().filter(|value| !value.trim().is_empty()))
    .and_then(|value| language_subtag(&value))
}

/// The bare language subtag of a POSIX/BCP47 locale token (`"en_ZA.UTF-8"` ->
/// `"en"`, `"pt-BR"` -> `"pt"`), `None` when empty or the special `C`/`POSIX`
/// locales (which carry no language).
fn language_subtag(locale: &str) -> Option<String> {
  let language: String = locale
    .trim()
    .chars()
    .take_while(|&c| c.is_ascii_alphabetic())
    .collect::<String>()
    .to_ascii_lowercase();
  match language.as_str() {
    "" | "c" | "posix" => None,
    _ => Some(language),
  }
}

// ---- Stable usage tokens (§8.1): lowercase, no spaces/slash/at/digits. -------

/// The stable, parameter-free usage token for a route, lowercased for
/// consistency with the dotted `sub_section` tokens (§8.1). Pairs with
/// [`crate::app`]'s `Route::name()` CamelCase constant so id-carrying variants
/// never leak an id.
pub fn route_token(name: &str) -> String {
  name.to_ascii_lowercase()
}

/// The stable `feature_toggle` token: the feature/sub-feature config key,
/// dotted as `group.sub` for a sub-feature so it matches the §6 contract
/// examples (`wallet.budget`). The telemetry toggles themselves are NOT routed
/// here (they live in `TelemetryConfig`, not the feature flags), so this never
/// emits a telemetry token.
pub fn feature_token(group_key: &str, sub_key: Option<&str>) -> String {
  match sub_key {
    Some(sub) => format!("{group_key}.{sub}"),
    None => group_key.to_owned(),
  }
}

/// Whether `token` satisfies the pinned usage/sub_section token shape (§8.1, the
/// privacy AC): non-empty and free of spaces, `/`, `@`, and any digit. Casing is
/// the caller's responsibility (every token capture lowercases). Shared with the
/// `app` token-shape test so the route/sub_section/feature token universe is
/// checked against one rule.
pub fn is_well_formed_token(token: &str) -> bool {
  !token.is_empty()
    && !token
      .chars()
      .any(|c| c == ' ' || c == '/' || c == '@' || c.is_ascii_digit())
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

  // ---- token helpers: stable, lowercase, contract-shaped. ----

  #[test]
  fn route_token_lowercases_the_camelcase_route_name() {
    assert_eq!(route_token("CharacterDetail"), "characterdetail");
    assert_eq!(route_token("Wallet"), "wallet");
  }

  #[test]
  fn feature_token_dots_a_sub_feature_under_its_group() {
    assert_eq!(feature_token("wallet", Some("budget")), "wallet.budget");
    assert_eq!(feature_token("wallet", None), "wallet");
  }

  #[test]
  fn is_well_formed_token_rejects_spaces_slash_at_and_digits() {
    assert!(is_well_formed_token("wallet"));
    assert!(is_well_formed_token("wallet.budget"));
    assert!(is_well_formed_token("asset_tracking.inventory"));
    assert!(!is_well_formed_token(""));
    assert!(!is_well_formed_token("wallet budget"));
    assert!(!is_well_formed_token("wallet/budget"));
    assert!(!is_well_formed_token("wallet@budget"));
    assert!(!is_well_formed_token("wallet2"));
  }

  // ---- environment probing: version/locale reduction is major/language-only. ----

  #[test]
  fn major_component_keeps_only_the_leading_digit_run() {
    assert_eq!(major_component("15.5").as_deref(), Some("15"));
    assert_eq!(major_component("22.04").as_deref(), Some("22"));
    assert_eq!(major_component("10.0.19045").as_deref(), Some("10"));
    assert_eq!(major_component("rolling"), None);
    assert_eq!(major_component(""), None);
  }

  // ---- per-platform os_version_major parse seams (pure, host-independent). ----

  #[test]
  fn parse_sw_vers_trims_the_product_version_line() {
    // macOS `sw_vers -productVersion` typically prints the bare version + newline.
    assert_eq!(parse_sw_vers("15.5\n").as_deref(), Some("15.5"));
    assert_eq!(parse_sw_vers("  14.4.1  ").as_deref(), Some("14.4.1"));
    // Reducing the parsed value yields the major component.
    assert_eq!(
      major_component(&parse_sw_vers("15.5\n").unwrap()).as_deref(),
      Some("15")
    );
    // Blank / whitespace-only output => nothing to parse.
    assert_eq!(parse_sw_vers(""), None);
    assert_eq!(parse_sw_vers("   \n"), None);
  }

  #[test]
  fn parse_windows_ver_extracts_the_token_after_the_version_marker() {
    let raw = "\r\nMicrosoft Windows [Version 10.0.19045.0]\r\n";
    assert_eq!(parse_windows_ver(raw).as_deref(), Some("10.0.19045.0]"));
    // The major reduction stops at the first non-digit, dropping the trailing `]`.
    assert_eq!(major_component(&parse_windows_ver(raw).unwrap()).as_deref(), Some("10"));
    // No "Version " marker => no token.
    assert_eq!(parse_windows_ver("some unexpected banner"), None);
    assert_eq!(parse_windows_ver(""), None);
  }

  #[test]
  fn parse_os_release_version_id_reads_the_unquoted_value() {
    let release = "NAME=\"Ubuntu\"\nVERSION_ID=\"22.04\"\nPRETTY_NAME=\"Ubuntu 22.04 LTS\"\n";
    assert_eq!(parse_os_release_version_id(release).as_deref(), Some("22.04"));
    assert_eq!(
      major_component(&parse_os_release_version_id(release).unwrap()).as_deref(),
      Some("22")
    );
    // Unquoted values (e.g. some rolling distros) are read verbatim.
    assert_eq!(parse_os_release_version_id("VERSION_ID=36\n").as_deref(), Some("36"));
    // A rolling release with no numeric id still parses; the major reduction is
    // what yields `None` for the caller.
    assert_eq!(
      parse_os_release_version_id("VERSION_ID=rolling").as_deref(),
      Some("rolling")
    );
    assert_eq!(major_component("rolling"), None);
    // Missing key => no value.
    assert_eq!(parse_os_release_version_id("NAME=\"Arch Linux\"\n"), None);
    assert_eq!(parse_os_release_version_id(""), None);
  }

  #[test]
  fn os_version_major_probes_the_live_host_without_panicking() {
    // Drives the real per-platform branch on the test host (the macOS `sw_vers`
    // path here). Best-effort: either a numeric major or `None`, never empty.
    if let Some(major) = os_version_major() {
      assert!(
        major.chars().all(|c| c.is_ascii_digit()),
        "major is digits only: {major}"
      );
      assert!(!major.is_empty());
    }
  }

  #[test]
  fn language_subtag_drops_the_region_and_encoding() {
    assert_eq!(language_subtag("en_ZA.UTF-8").as_deref(), Some("en"));
    assert_eq!(language_subtag("pt-BR").as_deref(), Some("pt"));
    assert_eq!(language_subtag("EN").as_deref(), Some("en"));
    assert_eq!(language_subtag("C"), None);
    assert_eq!(language_subtag("POSIX"), None);
    assert_eq!(language_subtag(""), None);
  }

  #[test]
  fn environment_stream_fills_os_arch_and_falls_back_to_unknown_for_a_missing_display() {
    let env = environment_stream(None);
    assert_eq!(env.os, std::env::consts::OS);
    assert_eq!(env.arch, std::env::consts::ARCH);
    assert_eq!(env.display, UNKNOWN);
    // os_version / locale are best-effort: either a probed value or the sentinel,
    // but never empty.
    assert!(!env.os_version.is_empty());
    assert!(!env.locale.is_empty());
  }

  #[test]
  fn environment_stream_reports_the_captured_primary_window_size() {
    let env = environment_stream(Some("2560x1440"));
    assert_eq!(env.display, "2560x1440");
  }

  // ---- load_ms timer: opened by view_open, closed by the first matching paint. ----

  #[test]
  fn nav_timer_records_a_view_load_on_the_first_paint_of_the_opened_route() {
    let collector = collector(config(true, true, true, true));
    {
      let mut buffers = collector.buffers.lock().unwrap();
      buffers.nav_started = Some(("wallet".to_owned(), std::time::Instant::now()));
    }
    // Simulate record_view_loaded("wallet") closing the armed timer.
    {
      let mut buffers = collector.buffers.lock().unwrap();
      let (opened, started) = buffers.nav_started.clone().unwrap();
      assert_eq!(opened, "wallet");
      let load_ms = started.elapsed().as_millis() as u64;
      buffers.nav_started = None;
      buffers.views.push(PerformanceViewEntry {
        name: opened,
        load_ms,
        frame_p95_ms: 0,
      });
    }
    let buffers = collector.buffers.lock().unwrap();
    assert_eq!(buffers.views.len(), 1);
    assert_eq!(buffers.views[0].name, "wallet");
    assert!(buffers.nav_started.is_none(), "timer is consumed on close");
  }
}
