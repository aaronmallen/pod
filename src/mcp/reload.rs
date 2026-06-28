use std::sync::Mutex;

use iced::{Subscription, futures::Stream};

static SENDER: Mutex<Option<Sender>> = Mutex::new(None);

type Sender = iced::futures::channel::mpsc::Sender<DataChanged>;

/// A coarse "an MCP write changed the database" marker. The app treats it as a cue to refresh the
/// active view; it carries no detail because the write may touch data outside what is on screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataChanged;

pub fn subscription() -> Subscription<DataChanged> {
  Subscription::run(stream)
}

pub fn signal() {
  if let Ok(mut guard) = SENDER.lock()
    && let Some(tx) = guard.as_mut()
  {
    let _ = tx.try_send(DataChanged);
  }
}

fn stream() -> impl Stream<Item = DataChanged> {
  iced::stream::channel(16, |tx: Sender| async move {
    if let Ok(mut guard) = SENDER.lock() {
      *guard = Some(tx);
    }
    std::future::pending::<()>().await;
  })
}
