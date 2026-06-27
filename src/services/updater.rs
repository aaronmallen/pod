use std::time::Duration;

use cargo_packager_updater::{Config as UpdaterConfig, Error, Update, check_update, semver::Version, url::Url};
use tokio::{
  sync::{mpsc, watch},
  time::{self, MissedTickBehavior},
};

pub const CHECK_INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);

const COMMAND_BUFFER: usize = 8;

// Derived from the crate repository at compile time so it can't silently go
// missing: as an optional `option_env!` endpoint it was unset in the release
// build, so `from_env` returned `None` and 0.5.0/0.5.1 shipped with the updater
// disabled.
const UPDATE_ENDPOINT: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases/latest/download/latest.json");

#[derive(Clone, Debug)]
pub struct Config {
  pub endpoints: Vec<Url>,
  pub pubkey: String,
}

impl Config {
  pub fn from_env() -> Option<Self> {
    let pubkey = option_env!("PACKAGER_PUBLIC_KEY")?.trim().to_owned();
    if pubkey.is_empty() {
      return None;
    }
    let endpoint = Url::parse(UPDATE_ENDPOINT).ok()?;
    Some(Self {
      endpoints: vec![endpoint],
      pubkey,
    })
  }

  fn into_updater_config(self) -> UpdaterConfig {
    UpdaterConfig {
      endpoints: self.endpoints,
      pubkey: self.pubkey,
      ..Default::default()
    }
  }
}

#[derive(Clone, Debug)]
pub struct Handle {
  checks: watch::Receiver<u64>,
  commands: mpsc::Sender<Command>,
  state: watch::Receiver<State>,
}

impl Handle {
  pub fn apply(&self) {
    let _ = self.commands.try_send(Command::Apply);
  }

  pub fn check(&self) {
    let _ = self.commands.try_send(Command::Check);
  }

  pub fn restart(&self) {
    let _ = self.commands.try_send(Command::Restart);
  }

  pub fn shutdown(&self) {
    let _ = self.commands.try_send(Command::Shutdown);
  }

  #[expect(dead_code)]
  pub fn state(&self) -> State {
    self.state.borrow().clone()
  }

  // Counts completed checks so a fast "no update" can be told apart from the
  // pre-check Idle default without waiting out the preflight timeout.
  pub fn subscribe_checks(&self) -> watch::Receiver<u64> {
    self.checks.clone()
  }

  pub fn subscribe(&self) -> watch::Receiver<State> {
    self.state.clone()
  }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum State {
  Downloading {
    version: String,
  },
  Error {
    message: String,
  },
  #[default]
  Idle,
  ReadyToRestart {
    version: String,
  },
  UpdateAvailable {
    version: String,
  },
}

impl State {
  pub fn version(&self) -> Option<&str> {
    match self {
      State::UpdateAvailable {
        version,
      }
      | State::Downloading {
        version,
      }
      | State::ReadyToRestart {
        version,
      } => Some(version),
      State::Idle
      | State::Error {
        ..
      } => None,
    }
  }
}

#[derive(Debug)]
enum Command {
  Apply,
  Check,
  Restart,
  Shutdown,
}

struct Engine {
  checks: watch::Sender<u64>,
  config: Config,
  pending: Option<Update>,
  state: watch::Sender<State>,
}

impl Engine {
  async fn apply(&mut self) {
    let Some(update) = self.pending.take() else {
      tracing::debug!(target: "pod::updater", "apply requested with no pending update");
      return;
    };
    let version = update.version.clone();
    self.set(State::Downloading {
      version: version.clone(),
    });
    let result = tokio::task::spawn_blocking(move || update.download_and_install()).await;
    match result {
      Ok(Ok(())) => {
        tracing::info!(target: "pod::updater", %version, "update installed; restart pending");
        self.set(State::ReadyToRestart {
          version,
        });
      }
      Ok(Err(error)) => {
        tracing::warn!(target: "pod::updater", %error, "update install failed");
        self.set(State::Error {
          message: format!("update install failed: {error}"),
        });
      }
      Err(error) => {
        tracing::error!(target: "pod::updater", %error, "update install task panicked");
        self.set(State::Error {
          message: format!("update install task failed: {error}"),
        });
      }
    }
  }

  fn apply_check_result(&mut self, result: Result<Result<Option<Update>, Error>, tokio::task::JoinError>) {
    match result {
      Ok(Ok(Some(update))) => {
        tracing::info!(target: "pod::updater", version = %update.version, "update available");
        self.set(State::UpdateAvailable {
          version: update.version.clone(),
        });
        self.pending = Some(update);
      }
      Ok(Ok(None)) => {
        tracing::debug!(target: "pod::updater", "no update available");
        self.pending = None;
        if matches!(*self.state.borrow(), State::UpdateAvailable { .. } | State::Idle) {
          self.set(State::Idle);
        }
      }
      Ok(Err(error)) => {
        tracing::warn!(target: "pod::updater", %error, "update check failed");
        self.set(State::Error {
          message: format!("update check failed: {error}"),
        });
      }
      Err(error) => {
        tracing::error!(target: "pod::updater", %error, "update check task panicked");
        self.set(State::Error {
          message: format!("update check task failed: {error}"),
        });
      }
    }
  }

  async fn check(&mut self) {
    let current = match Version::parse(env!("CARGO_PKG_VERSION")) {
      Ok(version) => version,
      Err(error) => {
        tracing::error!(target: "pod::updater", %error, "current version is not valid semver");
        self.set(State::Error {
          message: format!("invalid current version: {error}"),
        });
        return;
      }
    };
    let config = self.config.clone().into_updater_config();
    let result = tokio::task::spawn_blocking(move || check_update(current, config)).await;
    self.apply_check_result(result);
    self.checks.send_modify(|count| *count += 1);
  }

  fn restart(&self) {
    if matches!(*self.state.borrow(), State::ReadyToRestart { .. }) {
      tracing::info!(target: "pod::updater", "restarting into new release");
      crate::app::restart();
    } else {
      tracing::debug!(target: "pod::updater", "restart requested but no installed update is ready");
    }
  }

  async fn run(mut self, mut commands: mpsc::Receiver<Command>) {
    let mut ticker = time::interval(CHECK_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
      tokio::select! {
        _ = ticker.tick() => self.check().await,
        command = commands.recv() => match command {
          Some(Command::Apply) => self.apply().await,
          Some(Command::Check) => self.check().await,
          Some(Command::Restart) => self.restart(),
          Some(Command::Shutdown) | None => break,
        },
      }
    }
  }

  fn set(&self, state: State) {
    let _ = self.state.send(state);
  }
}

pub fn spawn(config: Config) -> Handle {
  let (command_tx, command_rx) = mpsc::channel(COMMAND_BUFFER);
  let (checks_tx, checks_rx) = watch::channel(0);
  let (state_tx, state_rx) = watch::channel(State::Idle);
  let engine = Engine {
    checks: checks_tx,
    config,
    pending: None,
    state: state_tx,
  };
  tokio::spawn(engine.run(command_rx));
  Handle {
    checks: checks_rx,
    commands: command_tx,
    state: state_rx,
  }
}

#[cfg(test)]
pub(crate) fn detached_handle() -> Handle {
  let (command_tx, _command_rx) = mpsc::channel(COMMAND_BUFFER);
  let (_checks_tx, checks_rx) = watch::channel(0);
  let (_state_tx, state_rx) = watch::channel(State::Idle);
  Handle {
    checks: checks_rx,
    commands: command_tx,
    state: state_rx,
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;
  use tokio::sync::{mpsc, watch};

  use super::*;

  fn engine() -> (Engine, watch::Receiver<State>) {
    let (checks_tx, _checks_rx) = watch::channel(0);
    let (state_tx, state_rx) = watch::channel(State::Idle);
    let engine = Engine {
      checks: checks_tx,
      config: Config {
        endpoints: vec![Url::parse("https://example.invalid/latest.json").unwrap()],
        pubkey: "test-key".to_owned(),
      },
      pending: None,
      state: state_tx,
    };
    (engine, state_rx)
  }

  #[tokio::test]
  async fn apply_without_pending_update_is_a_no_op() {
    let (mut engine, rx) = engine();
    engine.apply().await;
    assert!(
      !rx.has_changed().unwrap(),
      "apply with no pending update must not transition state"
    );
  }

  #[tokio::test]
  async fn handle_check_transitions_to_error_on_unreachable_endpoint() {
    let handle = spawn(Config {
      endpoints: vec![Url::parse("http://127.0.0.1:1/latest.json").unwrap()],
      pubkey: "untrusted".to_owned(),
    });
    let mut state = handle.subscribe();
    let outcome = tokio::time::timeout(Duration::from_secs(30), async {
      loop {
        state.changed().await.unwrap();
        let current = state.borrow().clone();
        if !matches!(current, State::Idle) {
          break current;
        }
      }
    })
    .await
    .expect("updater should report a non-idle state quickly");
    assert!(
      matches!(outcome, State::Error { .. }),
      "unreachable endpoint must surface as Error, got {outcome:?}"
    );
  }

  #[tokio::test]
  async fn check_completion_counter_advances_once_the_check_resolves() {
    let handle = spawn(Config {
      endpoints: vec![Url::parse("http://127.0.0.1:1/latest.json").unwrap()],
      pubkey: "untrusted".to_owned(),
    });
    let mut checks = handle.subscribe_checks();
    handle.check();
    let count = tokio::time::timeout(Duration::from_secs(30), async {
      checks.changed().await.unwrap();
      *checks.borrow_and_update()
    })
    .await
    .expect("a completed check must bump the counter");

    assert!(count >= 1, "completed check should advance the counter, got {count}");
  }

  #[tokio::test]
  async fn handle_commands_do_not_panic_when_task_is_gone() {
    let (command_tx, command_rx) = mpsc::channel(COMMAND_BUFFER);
    let (_checks_tx, checks_rx) = watch::channel(0);
    let (_state_tx, state_rx) = watch::channel(State::Idle);
    let handle = Handle {
      checks: checks_rx,
      commands: command_tx,
      state: state_rx,
    };
    drop(command_rx);
    handle.check();
    handle.apply();
    handle.restart();
  }

  #[tokio::test]
  async fn restart_does_nothing_when_not_ready() {
    let (engine, _rx) = engine();
    engine.restart();
  }

  #[test]
  fn set_publishes_state_to_watchers() {
    let (engine, mut rx) = engine();
    engine.set(State::UpdateAvailable {
      version: "9.9.9".to_owned(),
    });
    assert!(rx.has_changed().unwrap());
    assert_eq!(
      *rx.borrow_and_update(),
      State::UpdateAvailable {
        version: "9.9.9".to_owned()
      }
    );
  }

  #[tokio::test]
  async fn shutdown_sends_the_loop_breaking_command() {
    let (commands, mut rx) = mpsc::channel(4);
    let (_checks_tx, checks) = watch::channel(0);
    let (_state_tx, state) = watch::channel(State::Idle);
    let handle = Handle {
      checks,
      commands,
      state,
    };

    handle.shutdown();

    assert!(
      matches!(rx.recv().await, Some(Command::Shutdown)),
      "shutdown sends the Shutdown command that breaks the updater run loop"
    );
  }

  #[test]
  fn state_defaults_to_idle() {
    assert_eq!(State::default(), State::Idle);
  }

  #[test]
  fn state_exposes_target_version_only_when_relevant() {
    assert_eq!(State::Idle.version(), None);
    assert_eq!(
      State::Error {
        message: "boom".to_owned()
      }
      .version(),
      None
    );
    assert_eq!(
      State::UpdateAvailable {
        version: "1.2.3".to_owned()
      }
      .version(),
      Some("1.2.3")
    );
    assert_eq!(
      State::Downloading {
        version: "1.2.3".to_owned()
      }
      .version(),
      Some("1.2.3")
    );
    assert_eq!(
      State::ReadyToRestart {
        version: "1.2.3".to_owned()
      }
      .version(),
      Some("1.2.3")
    );
  }
}
