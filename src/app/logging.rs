use super::*;

pub(super) type FileFilterReloadHandle =
  OnceLock<tracing_subscriber::reload::Handle<tracing_subscriber::EnvFilter, tracing_subscriber::Registry>>;

pub(super) fn apply_log_level(level: config::LogLevel) {
  let filter = file_filter(level);
  let Some(handle) = file_filter_reload_handle().get() else {
    return;
  };
  if let Err(error) = handle.reload(tracing_subscriber::EnvFilter::new(&filter)) {
    tracing::warn!(target: "pod::lifecycle", %error, "could not apply the new log level live");
  }
}

pub(super) fn file_filter(level: config::LogLevel) -> String {
  let pod = match level {
    config::LogLevel::Normal => "debug",
    config::LogLevel::Quiet => "info",
    config::LogLevel::Verbose => "trace",
  };
  format!("pod={pod},{FILE_FILTER_CLAMP}")
}

pub(super) fn file_filter_reload_handle() -> &'static FileFilterReloadHandle {
  static HANDLE: OnceLock<FileFilterReloadHandle> = OnceLock::new();
  HANDLE.get_or_init(OnceLock::new)
}

pub(super) fn init_tracing(
  log_dir: &std::path::Path,
  log_level: config::LogLevel,
) -> Option<tracing_appender::non_blocking::WorkerGuard> {
  use tracing_subscriber::{Layer as _, filter::EnvFilter, fmt, prelude::*, reload};

  let console_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(CONSOLE_DEFAULT_FILTER));
  let console_layer = fmt::layer().compact().with_filter(console_filter);

  let active_file_filter = file_filter(log_level);

  let (file_layer, guard) = match std::fs::create_dir_all(log_dir) {
    Ok(()) => {
      let appender = tracing_appender::rolling::Builder::new()
        .filename_prefix("pod")
        .filename_suffix("log")
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        // Sits past the retention window (which spares the cutoff day itself) so this count cap can
        // never cut the Last 7 days export short; the startup sweep is what actually expires files.
        .max_log_files(usize::try_from(retention::RETENTION_DAYS).unwrap_or(7) + 1)
        .build(log_dir);
      match appender {
        Ok(appender) => {
          let (writer, guard) = tracing_appender::non_blocking(appender);
          let (filter, handle) = reload::Layer::new(EnvFilter::new(&active_file_filter));
          let _ = file_filter_reload_handle().set(handle);
          let layer = fmt::layer()
            .json()
            .with_ansi(false)
            .with_writer(writer)
            .with_filter(filter);
          (Some(layer), Some(guard))
        }
        Err(error) => {
          eprintln!(
            "pod: could not open log file appender in {}: {error}",
            log_dir.display()
          );
          (None, None)
        }
      }
    }
    Err(error) => {
      eprintln!("pod: could not create log directory {}: {error}", log_dir.display());
      (None, None)
    }
  };

  // Crash context_log ring (spec mmmzstpq §5.5/§8.4): a bounded in-memory ring
  // fed by every event, applying the allow-list AT INGEST so nothing unscrubbed
  // is ever held. The panic hook reads this ring, never the rolling file.
  let ring_layer = telemetry::crash::RingLayer.with_filter(EnvFilter::new(file_filter(log_level)));

  let _ = tracing_subscriber::registry()
    .with(file_layer)
    .with(console_layer)
    .with(ring_layer)
    .try_init();

  tracing::info!(
    target: "pod::lifecycle",
    version = env!("CARGO_PKG_VERSION"),
    log_dir = %log_dir.display(),
    console_filter = CONSOLE_DEFAULT_FILTER,
    file_filter = %active_file_filter,
    "pod starting up"
  );

  guard
}

/// Installs a process-wide panic hook that records every panic into the tracing JSON file log before
/// the default hook (and the `windows_subsystem = "windows"` console detachment) swallows the stderr
/// message. Without this, a panic in any spawned task — notably the sync engine's top-level task —
/// dies silently and leaves no trace in an exported field log.
///
/// Must be called after [`init_tracing`] so the subscriber already exists, and only on the non-test
/// boot path: tests must not mutate the global panic hook (it would clobber the harness's own hook
/// and break `#[should_panic]` / unwinding diagnostics).
#[cfg(not(test))]
pub(super) fn install_panic_hook() {
  let default_hook = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |info| {
    log_panic(info);
    default_hook(info);
  }));
}

#[cfg(test)]
pub(super) fn install_panic_hook() {}

pub(super) fn log_panic(info: &std::panic::PanicHookInfo<'_>) {
  let message = info
    .payload()
    .downcast_ref::<&str>()
    .copied()
    .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
    .unwrap_or("<non-string panic payload>");
  let location = info
    .location()
    .map(ToString::to_string)
    .unwrap_or_else(|| "<unknown>".to_owned());
  let backtrace = std::backtrace::Backtrace::force_capture();
  tracing::error!(
    target: "pod::lifecycle",
    panic_message = message,
    panic_location = location,
    panic_backtrace = %backtrace,
    "the process panicked",
  );

  // Crash pipeline (spec mmmzstpq §8.4): synchronously append one scrubbed
  // NDJSON record to the on-disk buffer for next-launch delivery. This is the
  // last thing the hook does and is fully best-effort: it never re-panics, does
  // no network / async, and no-ops when telemetry is opted out or unbuilt.
  telemetry::crash::capture(
    message,
    info.location().map(|_| location.as_str()),
    &backtrace.to_string(),
  );
}

#[cfg(test)]
mod tests {
  use super::*;

  mod crash_visibility {
    use std::sync::{Arc, Mutex};

    use tracing::{Event, Subscriber, field::Visit};
    use tracing_subscriber::{
      Layer,
      filter::EnvFilter,
      layer::{Context, SubscriberExt as _},
      registry,
    };

    use super::*;

    #[derive(Clone, Default)]
    struct CaptureLayer {
      messages: Arc<Mutex<Vec<String>>>,
    }

    struct MessageVisitor<'a>(&'a mut Option<String>);

    impl Visit for MessageVisitor<'_> {
      fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
          *self.0 = Some(format!("{value:?}"));
        }
      }
    }

    impl<S: Subscriber> Layer<S> for CaptureLayer {
      fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut message = None;
        event.record(&mut MessageVisitor(&mut message));
        if let Some(message) = message {
          self.messages.lock().expect("capture buffer").push(message);
        }
      }
    }

    fn passes_file_filter(log_level: config::LogLevel, emit: impl FnOnce()) -> bool {
      let layer = CaptureLayer::default();
      let messages = layer.messages.clone();
      let filtered = layer.with_filter(EnvFilter::new(file_filter(log_level)));
      tracing::subscriber::with_default(registry().with(filtered), emit);
      !messages.lock().expect("capture buffer").is_empty()
    }

    #[test]
    fn it_filters_pod_debug_out_at_quiet() {
      assert!(
        !passes_file_filter(config::LogLevel::Quiet, || {
          tracing::debug!(target: "pod::sync::engine", "event")
        }),
        "Quiet pins pod to INFO, so pod DEBUG must be filtered out"
      );

      assert!(
        passes_file_filter(config::LogLevel::Quiet, || {
          tracing::info!(target: "pod::sync::engine", "event")
        }),
        "Quiet must still admit pod INFO"
      );
    }

    #[test]
    fn it_hides_the_demoted_http_site_until_verbose() {
      let emit = || tracing::trace!(target: "pod::http", "request completed");

      assert!(
        !passes_file_filter(config::LogLevel::Quiet, emit),
        "the http per-request site must be silent at Quiet"
      );
      assert!(
        !passes_file_filter(config::LogLevel::Normal, emit),
        "the http per-request site must stay silent at Normal so the demotion keeps real signal afloat"
      );
      assert!(
        passes_file_filter(config::LogLevel::Verbose, emit),
        "the http per-request site must surface at Verbose for a deep-dive repro"
      );
    }

    #[test]
    fn it_hides_the_demoted_resolve_site_until_verbose() {
      let emit = || tracing::trace!(target: "pod::sync::jobs::resolve", "resolved item type from db");

      assert!(
        !passes_file_filter(config::LogLevel::Quiet, emit),
        "the resolve cache-hit site must be silent at Quiet"
      );
      assert!(
        !passes_file_filter(config::LogLevel::Normal, emit),
        "the resolve cache-hit site must stay silent at Normal so the demotion keeps real signal afloat"
      );
      assert!(
        passes_file_filter(config::LogLevel::Verbose, emit),
        "the resolve cache-hit site must surface at Verbose for a deep-dive repro"
      );
    }

    #[test]
    fn it_pins_sqlx_query_logging_to_warn_or_higher() {
      let captured = |level: tracing::Level| -> bool {
        let layer = CaptureLayer::default();
        let messages = layer.messages.clone();
        let filtered = layer.with_filter(EnvFilter::new(file_filter(config::LogLevel::default())));
        tracing::subscriber::with_default(registry().with(filtered), || match level {
          tracing::Level::TRACE => tracing::trace!(target: "sqlx::query", "stmt"),
          tracing::Level::DEBUG => tracing::debug!(target: "sqlx::query", "stmt"),
          tracing::Level::INFO => tracing::info!(target: "sqlx::query", "stmt"),
          tracing::Level::WARN => tracing::warn!(target: "sqlx::query", "stmt"),
          tracing::Level::ERROR => tracing::error!(target: "sqlx::query", "stmt"),
        });
        !messages.lock().expect("capture buffer").is_empty()
      };

      assert!(
        !captured(tracing::Level::TRACE),
        "sqlx::query TRACE must be filtered out"
      );
      assert!(
        !captured(tracing::Level::DEBUG),
        "sqlx::query DEBUG must be filtered out"
      );
      assert!(!captured(tracing::Level::INFO), "sqlx::query INFO must be filtered out");
      assert!(
        captured(tracing::Level::WARN),
        "sqlx::query WARN must pass (filter pins WARN-or-higher)"
      );
    }

    #[test]
    fn it_routes_a_panic_through_the_hook_into_tracing() {
      let layer = CaptureLayer::default();
      let messages = layer.messages.clone();

      let previous = std::panic::take_hook();
      std::panic::set_hook(Box::new(log_panic));

      tracing::subscriber::with_default(registry().with(layer), || {
        let _ = std::panic::catch_unwind(|| {
          fn run_sync_job() {
            panic!("simulated sync engine crash");
          }
          run_sync_job();
        });
      });

      std::panic::set_hook(previous);

      let captured = messages.lock().expect("capture buffer");
      assert!(
        captured.iter().any(|m| m.contains("the process panicked")),
        "the panic hook routed an ERROR event into tracing; captured: {captured:?}",
      );
    }
  }

  mod file_filter {
    use pretty_assertions::assert_eq;

    use super::*;

    fn clamp_of(filter: &str) -> &str {
      filter.split_once(',').expect("filter has a pod= prefix").1
    }

    #[test]
    fn it_keeps_every_dependency_clamp_identical_across_levels() {
      let quiet = file_filter(config::LogLevel::Quiet);
      let normal = file_filter(config::LogLevel::Normal);
      let verbose = file_filter(config::LogLevel::Verbose);

      assert_eq!(clamp_of(&quiet), FILE_FILTER_CLAMP);
      assert_eq!(clamp_of(&normal), FILE_FILTER_CLAMP);
      assert_eq!(clamp_of(&verbose), FILE_FILTER_CLAMP);
    }

    #[test]
    fn it_varies_only_the_pod_level_per_log_level() {
      assert_eq!(
        file_filter(config::LogLevel::Quiet),
        format!("pod=info,{FILE_FILTER_CLAMP}")
      );
      assert_eq!(
        file_filter(config::LogLevel::Normal),
        format!("pod=debug,{FILE_FILTER_CLAMP}")
      );
      assert_eq!(
        file_filter(config::LogLevel::Verbose),
        format!("pod=trace,{FILE_FILTER_CLAMP}")
      );
    }
  }

  mod init_tracing {
    use super::*;

    #[test]
    fn it_initializes_a_file_logger_under_a_writable_dir() {
      let dir = tempfile::tempdir().expect("temp dir");

      let guard = init_tracing(dir.path(), config::LogLevel::default());

      assert!(guard.is_some(), "a writable log dir yields a worker guard");
    }
  }
}
