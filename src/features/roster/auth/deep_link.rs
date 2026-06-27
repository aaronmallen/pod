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

static FOCUS_SENDER: Mutex<Option<FocusSender>> = Mutex::new(None);

static PENDING: Mutex<Option<String>> = Mutex::new(None);

static SENDER: Mutex<Option<Sender>> = Mutex::new(None);

#[cfg_attr(target_os = "macos", allow(dead_code))]
#[derive(Debug, Eq, PartialEq)]
enum Claim {
  Forwarded,
  Primary { pending: Option<String> },
}

type FocusSender = iced::futures::channel::mpsc::Sender<()>;

type Sender = iced::futures::channel::mpsc::Sender<String>;

pub fn subscription() -> Subscription<String> {
  Subscription::run(stream)
}

pub fn focus_subscription() -> Subscription<()> {
  Subscription::run(focus_stream)
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
pub fn deliver_focus() {
  tracing::debug!("received deep-link focus ping");
  if let Ok(mut guard) = FOCUS_SENDER.lock()
    && let Some(tx) = guard.as_mut()
  {
    let _ = tx.try_send(());
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
  let url = url_from_args();
  let claim = resolve_claim(
    url.clone(),
    single_instance::forward_to_primary,
    single_instance::request_focus,
  );
  breadcrumb(url.as_deref(), &claim);
  match claim {
    Claim::Forwarded => true,
    Claim::Primary {
      pending,
    } => {
      if let Some(url) = pending {
        set_pending(url);
      }
      false
    }
  }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn forward_or_claim() -> bool {
  false
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub fn release_lock() {
  single_instance::release_lock();
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn release_lock() {}

/// Writes a flushed launch record directly to `launch.log` rather than via `tracing`, because
/// `forward_or_claim` runs before the tracing subscriber is initialized and a forwarding instance
/// exits immediately — both paths would silently drop any buffered tracing output.
#[cfg(any(target_os = "linux", target_os = "windows"))]
fn breadcrumb(url: Option<&str>, claim: &Claim) {
  use std::io::Write as _;

  let dir = crate::config::log_dir();
  if std::fs::create_dir_all(&dir).is_err() {
    return;
  }
  let Ok(mut file) = std::fs::OpenOptions::new()
    .append(true)
    .create(true)
    .open(dir.join("launch.log"))
  else {
    return;
  };
  let _ = writeln!(
    file,
    "{} {}",
    chrono::Utc::now().to_rfc3339(),
    breadcrumb_message(url, claim)
  );
  let _ = file.flush();
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn breadcrumb_message(url: Option<&str>, claim: &Claim) -> String {
  match (claim, url) {
    (Claim::Forwarded, Some(url)) => {
      format!("[INFO] launch forwarded callback to the running primary url={url}")
    }
    (Claim::Forwarded, None) => "[INFO] launch focused the running primary (no callback url)".to_owned(),
    (
      Claim::Primary {
        pending: Some(url),
      },
      _,
    ) => {
      format!("[WARN] launch could not reach a primary; handling callback locally url={url}")
    }
    (
      Claim::Primary {
        pending: None,
      },
      _,
    ) => "[INFO] launch became the primary (no callback url)".to_owned(),
  }
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn resolve_claim(
  url: Option<String>,
  forward: impl FnOnce(&str) -> bool,
  request_focus: impl FnOnce() -> bool,
) -> Claim {
  match url {
    Some(url) => {
      if forward(&url) {
        Claim::Forwarded
      } else {
        Claim::Primary {
          pending: Some(url),
        }
      }
    }
    None => {
      if request_focus() {
        Claim::Forwarded
      } else {
        Claim::Primary {
          pending: None,
        }
      }
    }
  }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn url_from_args() -> Option<String> {
  let prefix = format!("{SCHEME}://");
  std::env::args().find(|arg| arg.starts_with(&prefix))
}

fn focus_stream() -> impl Stream<Item = ()> {
  iced::stream::channel(16, |tx: FocusSender| async move {
    if let Ok(mut guard) = FOCUS_SENDER.lock() {
      *guard = Some(tx);
    }
    std::future::pending::<()>().await;
  })
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

  mod breadcrumb_message {
    use super::*;

    #[test]
    fn it_records_an_info_breadcrumb_for_a_forwarded_callback() {
      let url = format!("{SCHEME}://callback");

      let line = breadcrumb_message(Some(&url), &Claim::Forwarded);

      assert!(line.contains("[INFO]"));
      assert!(line.contains(&url));
    }

    #[test]
    fn it_records_an_info_breadcrumb_for_a_no_url_focus_and_for_becoming_primary() {
      let focused = breadcrumb_message(None, &Claim::Forwarded);
      let became_primary = breadcrumb_message(
        None,
        &Claim::Primary {
          pending: None,
        },
      );

      assert!(focused.contains("[INFO]"));
      assert!(became_primary.contains("[INFO]"));
    }

    #[test]
    fn it_warns_when_a_forward_falls_through_to_local_handling() {
      let url = format!("{SCHEME}://callback");

      let line = breadcrumb_message(
        Some(&url),
        &Claim::Primary {
          pending: Some(url.clone()),
        },
      );

      assert!(line.contains("[WARN]"));
      assert!(line.contains(&url));
    }
  }

  mod resolve_claim {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_becomes_primary_when_a_no_url_launch_finds_no_running_primary() {
      let claim = resolve_claim(None, |_| unreachable!(), || false);

      assert_eq!(
        claim,
        Claim::Primary {
          pending: None
        }
      );
    }

    #[test]
    fn it_focuses_a_running_primary_when_a_no_url_launch_reaches_one() {
      let claim = resolve_claim(None, |_| unreachable!(), || true);

      assert_eq!(claim, Claim::Forwarded);
    }

    #[test]
    fn it_forwards_a_callback_url_to_the_primary() {
      let claim = resolve_claim(Some(format!("{SCHEME}://callback")), |_| true, || unreachable!());

      assert_eq!(claim, Claim::Forwarded);
    }

    #[test]
    fn it_keeps_the_url_and_becomes_primary_when_forwarding_fails() {
      let url = format!("{SCHEME}://callback");

      let claim = resolve_claim(Some(url.clone()), |_| false, || unreachable!());

      assert_eq!(
        claim,
        Claim::Primary {
          pending: Some(url)
        }
      );
    }
  }

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
