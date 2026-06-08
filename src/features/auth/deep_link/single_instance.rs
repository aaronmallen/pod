//! Single-instance arbitration over a per-data-dir local socket: the first process
//! binds and listens; a later OS-launched instance forwards its deep-link URL to the
//! primary and exits.

use std::{
  io::{Read, Write},
  thread,
};

use interprocess::local_socket::{
  GenericNamespaced, ListenerOptions, Stream, ToNsName,
  traits::{Listener as _, Stream as _},
};

use crate::config;

/// Sentinel sent over the socket to request a window raise rather than a URL delivery.
///
/// The `pod-deeplink:` prefix ensures this value can never be mistaken for a real
/// `eveauth-pod://` callback: `validate()` rejects it because it lacks the scheme prefix,
/// and `classify()` matches it by equality before falling through to `validate()`.
const FOCUS_PING: &str = "pod-deeplink:focus";

pub type PrimaryLock = interprocess::local_socket::Listener;

enum Signal {
  Focus,
  Url(String),
}

pub fn forward_to_primary(url: &str) -> bool {
  forward_to(&socket_name(), url)
}

pub fn request_focus() -> bool {
  forward_to(&socket_name(), FOCUS_PING)
}

pub fn spawn_listener(lock: PrimaryLock, deliver: impl Fn(String) + Send + 'static) {
  let _ = thread::Builder::new()
    .name("deeplink-listener".to_owned())
    .spawn(move || accept_loop(lock, deliver));
}

pub fn try_become_primary() -> Option<PrimaryLock> {
  bind(&socket_name())
}

fn accept_loop(listener: PrimaryLock, deliver: impl Fn(String)) {
  loop {
    match listener.accept() {
      Ok(mut stream) => {
        let mut payload = String::new();
        if stream.read_to_string(&mut payload).is_ok() {
          match classify(payload.trim()) {
            Some(Signal::Url(url)) => deliver(url),
            Some(Signal::Focus) => super::deliver_focus(),
            None => {}
          }
        }
      }
      Err(error) => {
        tracing::warn!(%error, "deep-link listener accept failed");
        break;
      }
    }
  }
}

fn bind(name: &str) -> Option<PrimaryLock> {
  let ns_name = name.to_ns_name::<GenericNamespaced>().ok()?;
  match ListenerOptions::new().name(ns_name).create_sync() {
    Ok(listener) => Some(listener),
    Err(error) => {
      tracing::debug!(%error, "deep-link single-instance bind failed; a primary already runs");
      None
    }
  }
}

fn classify(payload: &str) -> Option<Signal> {
  if payload == FOCUS_PING {
    return Some(Signal::Focus);
  }
  validate(payload).map(Signal::Url)
}

fn forward_to(name: &str, url: &str) -> bool {
  let Ok(ns_name) = name.to_ns_name::<GenericNamespaced>() else {
    return false;
  };
  let Ok(mut stream) = Stream::connect(ns_name) else {
    return false;
  };
  stream.write_all(url.as_bytes()).is_ok()
}

fn socket_name() -> String {
  let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
  sha2::Digest::update(&mut hasher, config::data_dir().to_string_lossy().as_bytes());
  let digest = sha2::Digest::finalize(hasher);
  let token: String = digest.iter().take(8).map(|byte| format!("{byte:02x}")).collect();
  format!("pod-deeplink-{token}.sock")
}

fn validate(payload: &str) -> Option<String> {
  let prefix = format!("{}://", super::SCHEME);
  payload.starts_with(&prefix).then(|| payload.to_owned())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn unique_name(tag: &str) -> String {
    format!("pod-deeplink-test-{tag}-{}.sock", std::process::id())
  }

  mod forward_and_listen {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_forwards_a_url_from_a_second_instance_to_the_primary_listener() {
      let name = unique_name("roundtrip");
      let (tx, rx) = std::sync::mpsc::channel();
      let lock = bind(&name).expect("first bind becomes the primary");
      let _ = thread::spawn(move || {
        accept_loop(lock, move |url| {
          let _ = tx.send(url);
        })
      });
      let url = format!("{}://callback?code=warm&state=fwd", super::super::super::SCHEME);

      let forwarded = forward_to(&name, &url);
      let delivered = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();

      assert!(forwarded, "the second instance reaches the primary");
      assert_eq!(delivered, url);
    }

    #[test]
    fn it_does_not_deliver_a_payload_that_lacks_the_scheme_prefix() {
      let name = unique_name("reject");
      let (tx, rx) = std::sync::mpsc::channel();
      let lock = bind(&name).expect("first bind becomes the primary");
      let _ = thread::spawn(move || {
        accept_loop(lock, move |url| {
          let _ = tx.send(url);
        })
      });

      let forwarded = forward_to(&name, "https://evil.example/callback?code=a&state=b");
      let delivered = rx.recv_timeout(std::time::Duration::from_millis(500));

      assert!(
        forwarded,
        "the write itself succeeds; rejection happens at the listener"
      );
      assert!(delivered.is_err(), "a non-eveauth-pod payload is never delivered");
    }

    #[test]
    fn a_second_bind_on_the_same_name_is_refused_as_addr_in_use() {
      let name = unique_name("lock");
      let _primary = bind(&name).expect("first bind becomes the primary");

      let second = bind(&name);

      assert!(
        second.is_none(),
        "a second bind on a held name fails (a primary already runs)"
      );
    }
  }

  mod classify {
    use super::*;

    #[test]
    fn it_recognizes_the_focus_ping() {
      assert!(matches!(classify(FOCUS_PING), Some(Signal::Focus)));
    }

    #[test]
    fn it_recognizes_an_eveauth_pod_url_as_a_callback() {
      let url = format!("{}://callback?code=a&state=b", super::super::super::SCHEME);

      assert!(matches!(classify(&url), Some(Signal::Url(delivered)) if delivered == url));
    }

    #[test]
    fn it_rejects_a_payload_that_is_neither_a_callback_nor_the_focus_ping() {
      assert!(classify("https://evil.example/callback").is_none());
      assert!(classify("eveauth-pod-evil://callback").is_none());
      assert!(classify("").is_none());
    }
  }

  mod validate {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_an_eveauth_pod_url() {
      let url = format!("{}://callback?code=a&state=b", super::super::super::SCHEME);

      assert_eq!(validate(&url), Some(url.clone()));
    }

    #[test]
    fn it_rejects_a_payload_without_the_scheme_prefix() {
      assert_eq!(validate("https://evil.example/callback?code=a&state=b"), None);
      assert_eq!(validate("eveauth-pod-evil://callback"), None);
      assert_eq!(validate(""), None);
    }
  }
}
