use tokio::sync::mpsc;

use super::{command::Command, subject::Subject};
use crate::config::FeatureFlags;

#[derive(Clone, Debug)]
pub struct Handle {
  commands: mpsc::UnboundedSender<Command>,
  restart: mpsc::UnboundedSender<()>,
}

impl Handle {
  pub fn new(commands: mpsc::UnboundedSender<Command>, restart: mpsc::UnboundedSender<()>) -> Self {
    Self {
      commands,
      restart,
    }
  }

  pub fn discover(&self) {
    let _ = self.commands.send(Command::Discover);
  }

  pub fn drain(&self) {
    let _ = self.commands.send(Command::Drain);
  }

  pub fn enroll(&self, subject: Subject) {
    let _ = self.commands.send(Command::Enroll(subject));
  }

  /// Signals the supervisor over a dedicated channel so a restart reaches it even when the engine
  /// task (and its `Command` loop) has already died.
  pub fn restart_sync(&self) {
    let _ = self.restart.send(());
  }

  pub fn run_now(&self, subject: Subject) {
    let _ = self.commands.send(Command::RunNow(subject));
  }

  pub fn set_features(&self, features: FeatureFlags) {
    let _ = self.commands.send(Command::SetFeatures(features));
  }

  pub fn shutdown(&self) {
    let _ = self.commands.send(Command::Shutdown);
  }

  pub fn withdraw(&self, subject: Subject) {
    let _ = self.commands.send(Command::Withdraw(subject));
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  #[tokio::test]
  async fn it_sends_a_data_free_drain_command() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (restart_tx, _restart_rx) = mpsc::unbounded_channel();
    let handle = Handle::new(tx, restart_tx);

    handle.drain();

    assert_eq!(
      rx.recv().await,
      Some(Command::Drain),
      "the nudge is a unit, data-free Drain command"
    );
  }

  #[tokio::test]
  async fn it_sends_a_restart_signal_on_the_dedicated_side_channel() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let (restart_tx, mut restart_rx) = mpsc::unbounded_channel();
    let handle = Handle::new(tx, restart_tx);

    handle.restart_sync();

    assert_eq!(
      restart_rx.recv().await,
      Some(()),
      "restart_sync signals the supervisor over its own channel, not the engine command loop"
    );
  }

  #[tokio::test]
  async fn it_sends_a_shutdown_command() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (restart_tx, _restart_rx) = mpsc::unbounded_channel();
    let handle = Handle::new(tx, restart_tx);

    handle.shutdown();

    assert_eq!(
      rx.recv().await,
      Some(Command::Shutdown),
      "shutdown sends the loop-breaking Shutdown command"
    );
  }

  #[tokio::test]
  async fn it_reliably_delivers_enroll_without_blocking_when_commands_are_backed_up() {
    // The consumer is not yet draining; a bounded channel of the old size would have dropped these.
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (restart_tx, _restart_rx) = mpsc::unbounded_channel();
    let handle = Handle::new(tx, restart_tx);

    for _ in 0..128 {
      handle.enroll(Subject::Character(7));
    }
    handle.run_now(Subject::Character(7));

    let mut enrolls = 0;
    let mut saw_run_now = false;
    while let Ok(command) = rx.try_recv() {
      match command {
        Command::Enroll(Subject::Character(7)) => enrolls += 1,
        Command::RunNow(Subject::Character(7)) => saw_run_now = true,
        other => panic!("unexpected command: {other:?}"),
      }
    }

    assert_eq!(
      enrolls, 128,
      "every enroll survives a backed-up channel, none silently dropped"
    );
    assert!(saw_run_now, "the trailing run-now is delivered too");
  }

  #[tokio::test]
  async fn it_sends_a_set_features_command() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (restart_tx, _restart_rx) = mpsc::unbounded_channel();
    let handle = Handle::new(tx, restart_tx);
    let flags: FeatureFlags = toml::from_str("wallet = false").unwrap();

    handle.set_features(flags);

    assert_eq!(
      rx.recv().await,
      Some(Command::SetFeatures(flags)),
      "set_features sends the SetFeatures command"
    );
  }
}
