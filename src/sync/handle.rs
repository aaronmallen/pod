use tokio::sync::mpsc;

use super::{command::Command, subject::Subject};

#[derive(Clone, Debug)]
pub struct Handle {
  commands: mpsc::Sender<Command>,
}

impl Handle {
  pub fn new(commands: mpsc::Sender<Command>) -> Self {
    Self {
      commands,
    }
  }

  pub fn discover(&self) {
    let _ = self.commands.try_send(Command::Discover);
  }

  pub fn drain(&self) {
    let _ = self.commands.try_send(Command::Drain);
  }

  pub fn enroll(&self, subject: Subject) {
    let _ = self.commands.try_send(Command::Enroll(subject));
  }

  pub fn run_now(&self, subject: Subject) {
    let _ = self.commands.try_send(Command::RunNow(subject));
  }

  pub fn withdraw(&self, subject: Subject) {
    let _ = self.commands.try_send(Command::Withdraw(subject));
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  #[tokio::test]
  async fn it_sends_a_data_free_drain_command() {
    let (tx, mut rx) = mpsc::channel(4);
    let handle = Handle::new(tx);

    handle.drain();

    assert_eq!(
      rx.recv().await,
      Some(Command::Drain),
      "the nudge is a unit, data-free Drain command"
    );
  }

  #[tokio::test]
  async fn it_does_not_block_or_panic_when_the_channel_is_full() {
    let (tx, _rx) = mpsc::channel(1);
    let handle = Handle::new(tx);
    handle.drain();

    handle.drain();
  }
}
