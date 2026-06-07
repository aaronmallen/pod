#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg_attr(target_os = "macos", allow(dead_code))]
mod single_instance;
#[cfg(target_os = "windows")]
mod windows;

use std::sync::Mutex;

use iced::{Subscription, futures::Stream};

pub const SCHEME: &str = "eveauth-pod";

type Sender = iced::futures::channel::mpsc::Sender<String>;

static PENDING: Mutex<Option<String>> = Mutex::new(None);
static SENDER: Mutex<Option<Sender>> = Mutex::new(None);

pub fn subscription() -> Subscription<String> {
  Subscription::run(stream)
}

pub fn deliver(url: String) {
  tracing::debug!("received deep-link callback");
  if let Ok(mut guard) = SENDER.lock()
    && let Some(tx) = guard.as_mut()
  {
    let _ = tx.try_send(url);
  }
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn set_pending(url: String) {
  if let Ok(mut guard) = PENDING.lock() {
    *guard = Some(url);
  }
}

pub fn install() {
  #[cfg(target_os = "macos")]
  macos::install();
  #[cfg(target_os = "windows")]
  windows::install();
  #[cfg(target_os = "linux")]
  linux::install();
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub fn forward_or_claim() -> bool {
  let Some(url) = url_from_args() else {
    return false;
  };
  if single_instance::forward_to_primary(&url) {
    return true;
  }
  set_pending(url);
  false
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn forward_or_claim() -> bool {
  false
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn url_from_args() -> Option<String> {
  let prefix = format!("{SCHEME}://");
  std::env::args().find(|arg| arg.starts_with(&prefix))
}

fn stream() -> impl Stream<Item = String> {
  iced::stream::channel(16, |mut tx: Sender| async move {
    if let Ok(mut guard) = SENDER.lock() {
      *guard = Some(tx.clone());
    }
    if let Ok(mut guard) = PENDING.lock()
      && let Some(url) = guard.take()
    {
      let _ = tx.try_send(url);
    }
    std::future::pending::<()>().await;
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  mod stream {
    use iced::futures::StreamExt as _;
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_drains_a_url_stashed_in_pending_before_the_sender_registers() {
      set_pending(format!("{SCHEME}://callback?code=cold&state=start"));

      let mut events = std::pin::pin!(stream());
      let delivered = events.next().await;

      assert_eq!(delivered, Some(format!("{SCHEME}://callback?code=cold&state=start")));
      assert!(PENDING.lock().unwrap().is_none(), "PENDING is drained, not left set");
    }
  }
}
