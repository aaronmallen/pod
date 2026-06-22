//! The data-changed (reload/invalidate) signal an MCP write tool raises after mutating the database.
//!
//! Write tools land in later specs, but the mechanism is defined here so they have a stable hook:
//! after a tool commits a write, it calls [`signal`], an iced [`Subscription`] delivers a
//! [`DataChanged`] marker into the update loop, and the app reloads whatever the open view shows.
//! The marker is deliberately coarse — "something an agent changed"; the app decides what to reload.

use std::sync::Mutex;

use iced::{Subscription, futures::Stream};

static SENDER: Mutex<Option<Sender>> = Mutex::new(None);

type Sender = iced::futures::channel::mpsc::Sender<DataChanged>;

/// A coarse "an MCP write changed the database" marker. The app treats it as a cue to refresh the
/// active view; it carries no detail because the write may touch data outside what is on screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataChanged;

/// The iced subscription that stashes the live sender so [`signal`] can reach the update loop.
pub fn subscription() -> Subscription<DataChanged> {
  Subscription::run(stream)
}

/// Raises the data-changed signal so any open GUI view reloads. Safe to call from any thread; a
/// no-op when no subscription is active yet.
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
