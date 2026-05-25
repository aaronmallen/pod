//! Background update-checking service.

use cargo_packager_updater::{Config, UpdaterBuilder, semver::Version};
use iced::Task;
use tracing::{info, warn};

const CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(4 * 60 * 60);

const UPDATE_ENDPOINT: &str = "https://github.com/aaronmallen/pod/releases/latest/download/latest.json";

/// State of the in-app update lifecycle.
#[derive(Clone, Debug, Default)]
pub enum UpdateState {
  /// App is downloading and installing the update.
  Downloading,
  /// Update failed; carries the error description.
  Error(String),
  #[default]
  /// No update in progress.
  Idle,
  /// Update has been applied; app needs to restart.
  ReadyToRestart,
  /// A newer version is available; carries the version string.
  UpdateAvailable(String),
}

/// Messages produced by the updater service.
#[derive(Clone, Debug)]
pub enum Message {
  /// The update was downloaded and installed; app should restart to apply.
  ApplyComplete,
  /// The update download or installation failed.
  ApplyFailed(String),
  /// User requested to download and install the available update.
  ApplyRequested,
  /// Update check completed; `Some(version)` if a newer version is available.
  CheckComplete(Option<String>),
  /// Update check encountered a network or parse error.
  CheckFailed,
  /// A periodic or startup check has been triggered.
  CheckRequested,
  /// User requested the app to restart into the newly installed version.
  RestartRequested,
}

/// Returns the interval between periodic update checks.
pub const fn check_interval() -> std::time::Duration {
  CHECK_INTERVAL
}

/// Downloads and silently installs the available update in a background thread.
pub fn apply() -> Task<Message> {
  Task::perform(apply_inner(), handle_apply_result)
}

/// Spawns a background update check against the release manifest.
pub fn check() -> Task<Message> {
  Task::perform(check_inner(), handle_check_result)
}

/// Relaunches the current executable then terminates the current process.
pub fn restart() {
  if let Ok(exe) = std::env::current_exe() {
    let _ = std::process::Command::new(exe).spawn();
  }
  std::process::exit(0);
}

async fn apply_inner() -> Result<(), String> {
  tokio::task::spawn_blocking(apply_blocking)
    .await
    .unwrap_or_else(|e| Err(format!("task join error: {e}")))
}

fn apply_blocking() -> Result<(), String> {
  let update = fetch_pending_update()?;
  update.download_and_install().map_err(|e| e.to_string())
}

fn fetch_pending_update() -> Result<cargo_packager_updater::Update, String> {
  build_updater()
    .map_err(|e| e.to_string())?
    .check()
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "no update available".to_string())
}

async fn check_inner() -> Result<Option<String>, String> {
  tokio::task::spawn_blocking(check_blocking)
    .await
    .unwrap_or_else(|e| Err(format!("task join error: {e}")))
}

fn check_blocking() -> Result<Option<String>, String> {
  let updater = build_updater().map_err(|e| e.to_string())?;
  updater
    .check()
    .map(|opt| opt.map(|u| u.version))
    .map_err(|e| e.to_string())
}

fn handle_apply_result(result: Result<(), String>) -> Message {
  match result {
    Ok(()) => {
      info!("update applied successfully");
      Message::ApplyComplete
    }
    Err(e) => {
      warn!(error = %e, "update apply failed");
      Message::ApplyFailed(e)
    }
  }
}

fn handle_check_result(result: Result<Option<String>, String>) -> Message {
  match result {
    Ok(Some(version)) => {
      info!(%version, "update available");
      Message::CheckComplete(Some(version))
    }
    Ok(None) => {
      info!("app is up to date");
      Message::CheckComplete(None)
    }
    Err(e) => {
      warn!(error = %e, "update check failed");
      Message::CheckFailed
    }
  }
}

fn build_updater() -> Result<cargo_packager_updater::Updater, cargo_packager_updater::Error> {
  let version: Version = env!("CARGO_PKG_VERSION")
    .parse()
    .expect("CARGO_PKG_VERSION is not valid semver");
  let config = Config {
    endpoints: vec![UPDATE_ENDPOINT.parse().expect("UPDATE_ENDPOINT is a valid URL")],
    pubkey: option_env!("PACKAGER_PUBLIC_KEY").unwrap_or_default().to_string(),
    windows: None,
  };
  UpdaterBuilder::new(version, config).build()
}
