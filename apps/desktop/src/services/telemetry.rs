pub mod collector;
pub mod contract;
pub mod crash;
pub mod pii;

use rand::Rng;

/// The app identity carried on every envelope, from the same build-time values
/// the contract documents (`CARGO_PKG_VERSION`, `POD_GIT_SHA`, `POD_BUILD_DATE`).
/// Shared by the collector and crash subsystems.
pub(super) fn app_identity() -> contract::App {
  contract::App {
    version: env!("CARGO_PKG_VERSION").to_owned(),
    git_sha: non_empty(option_env!("POD_GIT_SHA")),
    build_date: non_empty(option_env!("POD_BUILD_DATE")),
  }
}

/// `Some(trimmed)` when the build-time value is present and non-blank, else
/// `None` (so the optional `app.git_sha` / `app.build_date` keys are omitted).
pub(super) fn non_empty(value: Option<&str>) -> Option<String> {
  let value = value?.trim();
  (!value.is_empty()).then(|| value.to_owned())
}

/// Current RFC3339 UTC timestamp (the per-event `t`, the envelope `sent_at`, and
/// the crash `crashed_at`). Shared by the collector and crash subsystems.
pub(super) fn now_rfc3339() -> String {
  chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Generate the per-process session tag: `"s_"` followed by exactly 8 lowercase
/// hex chars, satisfying the Worker's `^s_[0-9a-f]{8}$` regex. Shared by the
/// collector and crash subsystems, but each caller generates its own tag: a
/// session tag is semantically per-subsystem even though the impl is shared (the
/// crash envelope must carry the crashed run's own tag).
pub(super) fn session_tag() -> String {
  let mut bytes = [0u8; 4];
  rand::rng().fill_bytes(&mut bytes);
  let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
  format!("s_{hex}")
}
