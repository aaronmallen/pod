//! On non-Windows platforms a filesystem-path socket is used instead of an abstract
//! namespace socket because Flatpak sandboxes cannot see the abstract namespace.

#[cfg(not(target_os = "windows"))]
use std::path::{Path, PathBuf};
use std::{
  io::{Read, Write},
  sync::{Arc, Mutex},
  thread,
};

#[cfg(not(target_os = "windows"))]
use interprocess::local_socket::{GenericFilePath, ToFsName};
#[cfg(target_os = "windows")]
use interprocess::local_socket::{GenericNamespaced, ToNsName};
use interprocess::local_socket::{
  ListenerOptions, Stream,
  traits::{Listener as _, Stream as _},
};

use crate::config;

/// Prefix for a pack-file path forwarded over the same socket as URL callbacks and focus pings.
///
/// `classify()` strips this prefix before falling through to `validate()`, so a stashed path
/// (even a `C:\...` path with its own colons) is never mistaken for an `eveauth-pod://` callback.
const FILE_SIGNAL_PREFIX: &str = "pod-deeplink:file:";

/// Sentinel sent over the socket to request a window raise rather than a URL delivery.
///
/// The `pod-deeplink:` prefix ensures this value can never be mistaken for a real
/// `eveauth-pod://` callback: `validate()` rejects it because it lacks the scheme prefix,
/// and `classify()` matches it by equality before falling through to `validate()`.
const FOCUS_PING: &str = "pod-deeplink:focus";

static HELD_LOCK: Mutex<Option<Arc<PrimaryLock>>> = Mutex::new(None);

pub type PrimaryLock = interprocess::local_socket::Listener;

enum Signal {
  File(String),
  Focus,
  Url(String),
}

pub fn forward_file_to_primary(path: &str) -> bool {
  signal(&format!("{FILE_SIGNAL_PREFIX}{path}"))
}

pub fn forward_to_primary(url: &str) -> bool {
  signal(url)
}

pub fn release_lock() {
  drop_held_lock();
  #[cfg(not(target_os = "windows"))]
  release_socket_file(&socket_path());
}

pub fn request_focus() -> bool {
  signal(FOCUS_PING)
}

pub fn spawn_listener(lock: PrimaryLock, deliver: impl Fn(String) + Send + 'static) {
  let lock = Arc::new(lock);
  if let Ok(mut guard) = HELD_LOCK.lock() {
    *guard = Some(Arc::clone(&lock));
  }
  let _ = thread::Builder::new()
    .name("deeplink-listener".to_owned())
    .spawn(move || accept_loop(&lock, deliver));
}

pub fn try_become_primary() -> Option<PrimaryLock> {
  acquire()
}

fn accept_loop(listener: &PrimaryLock, deliver: impl Fn(String)) {
  loop {
    match listener.accept() {
      Ok(mut stream) => {
        let mut payload = String::new();
        if stream.read_to_string(&mut payload).is_ok() {
          dispatch_signal(payload.trim(), &deliver);
        }
      }
      Err(error) => {
        tracing::warn!(%error, "deep-link listener accept failed");
        break;
      }
    }
  }
}

#[cfg(not(target_os = "windows"))]
fn acquire() -> Option<PrimaryLock> {
  bind(&socket_path())
}

#[cfg(target_os = "windows")]
fn acquire() -> Option<PrimaryLock> {
  bind(&socket_file_name())
}

#[cfg(not(target_os = "windows"))]
fn bind(path: &Path) -> Option<PrimaryLock> {
  match create_listener(path) {
    Ok(listener) => Some(listener),
    Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => recover_stale(path),
    Err(error) => {
      tracing::debug!(%error, "deep-link single-instance bind failed");
      None
    }
  }
}

#[cfg(target_os = "windows")]
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
  if let Some(path) = payload.strip_prefix(FILE_SIGNAL_PREFIX) {
    return Some(Signal::File(path.to_owned()));
  }
  validate(payload).map(Signal::Url)
}

#[cfg(not(target_os = "windows"))]
fn create_listener(path: &Path) -> std::io::Result<PrimaryLock> {
  let name = path.to_fs_name::<GenericFilePath>()?;
  ListenerOptions::new().name(name).create_sync()
}

fn dispatch_signal(payload: &str, deliver: &impl Fn(String)) {
  match classify(payload) {
    Some(Signal::File(path)) => {
      // Unlike a URL callback, delivering a file triggers no UI flow of its own, so raise
      // the window explicitly or the primary stays hidden behind other apps.
      super::deliver_file(path);
      super::deliver_focus();
    }
    Some(Signal::Url(url)) => deliver(url),
    Some(Signal::Focus) => super::deliver_focus(),
    None => {}
  }
}

fn drop_held_lock() {
  if let Ok(mut guard) = HELD_LOCK.lock() {
    let _ = guard.take();
  }
}

#[cfg(not(target_os = "windows"))]
fn forward_to(path: &Path, payload: &str) -> bool {
  let Ok(name) = path.to_fs_name::<GenericFilePath>() else {
    return false;
  };
  let Ok(mut stream) = Stream::connect(name) else {
    return false;
  };
  stream.write_all(payload.as_bytes()).is_ok()
}

#[cfg(target_os = "windows")]
fn forward_to(name: &str, payload: &str) -> bool {
  let Ok(ns_name) = name.to_ns_name::<GenericNamespaced>() else {
    return false;
  };
  let Ok(mut stream) = Stream::connect(ns_name) else {
    return false;
  };
  stream.write_all(payload.as_bytes()).is_ok()
}

/// Probes liveness before reclaiming a socket path that failed with `AddrInUse`.
///
/// `AddrInUse` on a filesystem socket only means the path exists — not that anyone is
/// listening.  A connect-test distinguishes a live primary (forward, return `None`) from
/// a leftover file after a crash (unlink, rebind).
#[cfg(not(target_os = "windows"))]
fn recover_stale(path: &Path) -> Option<PrimaryLock> {
  if socket_has_listener(path) {
    tracing::debug!("deep-link single-instance bind failed; a primary already runs");
    return None;
  }
  let _ = std::fs::remove_file(path);
  create_listener(path).ok()
}

#[cfg(not(target_os = "windows"))]
fn release_socket_file(path: &Path) {
  let _ = std::fs::remove_file(path);
}

#[cfg(not(target_os = "windows"))]
fn resolve_socket_dir(runtime: Option<PathBuf>, data_dir: PathBuf) -> PathBuf {
  runtime.unwrap_or(data_dir)
}

#[cfg(not(target_os = "windows"))]
fn signal(payload: &str) -> bool {
  forward_to(&socket_path(), payload)
}

#[cfg(target_os = "windows")]
fn signal(payload: &str) -> bool {
  forward_to(&socket_file_name(), payload)
}

fn socket_file_name() -> String {
  let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
  sha2::Digest::update(&mut hasher, config::data_dir().to_string_lossy().as_bytes());
  let digest = sha2::Digest::finalize(hasher);
  let token: String = digest.iter().take(8).map(|byte| format!("{byte:02x}")).collect();
  format!("pod-deeplink-{token}.sock")
}

#[cfg(not(target_os = "windows"))]
fn socket_has_listener(path: &Path) -> bool {
  path.to_fs_name::<GenericFilePath>().and_then(Stream::connect).is_ok()
}

#[cfg(not(target_os = "windows"))]
fn socket_path() -> PathBuf {
  resolve_socket_dir(dir_spec::runtime(), config::data_dir()).join(socket_file_name())
}

fn validate(payload: &str) -> Option<String> {
  let prefix = format!("{}://", super::SCHEME);
  payload.starts_with(&prefix).then(|| payload.to_owned())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[cfg(not(target_os = "windows"))]
  fn unique_socket(tag: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("temp dir for an isolated socket");
    let path = dir
      .path()
      .join(format!("pod-deeplink-test-{tag}-{}.sock", std::process::id()));
    (dir, path)
  }

  #[cfg(target_os = "windows")]
  fn unique_name(tag: &str) -> String {
    format!("pod-deeplink-test-{tag}-{}.sock", std::process::id())
  }

  mod classify {
    use super::*;

    #[test]
    fn it_recognizes_an_eveauth_pod_url_as_a_callback() {
      let url = format!("{}://callback?code=a&state=b", super::super::super::SCHEME);

      assert!(matches!(classify(&url), Some(Signal::Url(delivered)) if delivered == url));
    }

    #[test]
    fn it_recognizes_a_pack_file_signal_and_strips_the_prefix() {
      let payload = format!("{FILE_SIGNAL_PREFIX}/tmp/rules.pbr");

      assert!(matches!(classify(&payload), Some(Signal::File(path)) if path == "/tmp/rules.pbr"));
    }

    #[test]
    fn it_recognizes_the_focus_ping() {
      assert!(matches!(classify(FOCUS_PING), Some(Signal::Focus)));
    }

    #[test]
    fn it_rejects_a_payload_that_is_neither_a_callback_nor_the_focus_ping() {
      assert!(classify("https://evil.example/callback").is_none());
      assert!(classify("eveauth-pod-evil://callback").is_none());
      assert!(classify("").is_none());
    }

    #[test]
    fn it_drops_a_bare_file_path_that_lacks_the_signal_prefix() {
      assert!(classify("/tmp/rules.pbr").is_none());
      assert!(classify(r"C:\Users\me\plan.psp").is_none());
    }
  }

  mod dispatch_signal {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_delivers_a_callback_url_to_the_handler() {
      let url = format!("{}://callback?code=a&state=b", super::super::super::SCHEME);
      let (tx, rx) = std::sync::mpsc::channel();

      dispatch_signal(&url, &move |delivered| {
        let _ = tx.send(delivered);
      });

      assert_eq!(rx.recv().unwrap(), url);
    }

    #[test]
    fn it_ignores_a_payload_that_is_neither_a_callback_nor_a_ping() {
      let (tx, rx) = std::sync::mpsc::channel();

      dispatch_signal("https://evil.example/callback", &move |delivered| {
        let _ = tx.send(delivered);
      });

      assert!(rx.try_recv().is_err(), "an unrecognized payload reaches no handler");
    }
  }

  mod forward_and_listen {
    use pretty_assertions::assert_eq;

    use super::*;

    #[cfg(not(target_os = "windows"))]
    fn bind_listener(tag: &str) -> (tempfile::TempDir, PathBuf, PrimaryLock) {
      let (dir, path) = unique_socket(tag);
      let lock = bind(&path).expect("first bind becomes the primary");
      (dir, path, lock)
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn a_second_bind_on_the_same_name_is_refused_as_addr_in_use() {
      let name = unique_name("lock");
      let _primary = bind(&name).expect("first bind becomes the primary");

      let second = bind(&name);

      assert!(
        second.is_none(),
        "a second bind on a held name fails (a primary already runs)"
      );
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn it_does_not_deliver_a_payload_that_lacks_the_scheme_prefix() {
      let (_dir, path, lock) = bind_listener("reject");
      let (tx, rx) = std::sync::mpsc::channel();
      let _ = thread::spawn(move || {
        accept_loop(&lock, move |url| {
          let _ = tx.send(url);
        })
      });

      let forwarded = forward_to(&path, "https://evil.example/callback?code=a&state=b");
      let delivered = rx.recv_timeout(std::time::Duration::from_millis(500));

      assert!(
        forwarded,
        "the write itself succeeds; rejection happens at the listener"
      );
      assert!(delivered.is_err(), "a non-eveauth-pod payload is never delivered");
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn it_forwards_a_url_from_a_second_instance_to_the_primary_listener() {
      let (_dir, path, lock) = bind_listener("roundtrip");
      let (tx, rx) = std::sync::mpsc::channel();
      let _ = thread::spawn(move || {
        accept_loop(&lock, move |url| {
          let _ = tx.send(url);
        })
      });
      let url = format!("{}://callback?code=warm&state=fwd", super::super::super::SCHEME);

      let forwarded = forward_to(&path, &url);
      let delivered = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();

      assert!(forwarded, "the second instance reaches the primary");
      assert_eq!(delivered, url);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn it_forwards_a_url_from_a_second_instance_to_the_primary_listener() {
      let name = unique_name("roundtrip");
      let (tx, rx) = std::sync::mpsc::channel();
      let lock = bind(&name).expect("first bind becomes the primary");
      let _ = thread::spawn(move || {
        accept_loop(&lock, move |url| {
          let _ = tx.send(url);
        })
      });
      let url = format!("{}://callback?code=warm&state=fwd", super::super::super::SCHEME);

      let forwarded = forward_to(&name, &url);
      let delivered = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();

      assert!(forwarded, "the second instance reaches the primary");
      assert_eq!(delivered, url);
    }
  }

  #[cfg(not(target_os = "windows"))]
  mod release_socket_file {
    use super::*;

    #[test]
    fn it_frees_the_path_so_a_relaunch_can_rebind() {
      let (_dir, path) = unique_socket("handoff");
      let primary = bind(&path).expect("first bind becomes the primary");
      assert!(
        bind(&path).is_none(),
        "while the primary holds the socket a second bind is refused, not duplicated"
      );

      release_socket_file(&path);
      let relaunched = bind(&path);

      assert!(
        relaunched.is_some(),
        "releasing the socket file lets the relaunched process re-acquire the lock at the same path"
      );
      drop(primary);
    }
  }

  #[cfg(not(target_os = "windows"))]
  mod resolve_socket_dir {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_falls_back_to_the_data_dir_when_the_runtime_dir_is_unset() {
      let resolved = resolve_socket_dir(None, PathBuf::from("/home/me/.local/share/pod"));

      assert_eq!(resolved, PathBuf::from("/home/me/.local/share/pod"));
    }

    #[test]
    fn it_prefers_the_runtime_dir_when_present() {
      let resolved = resolve_socket_dir(
        Some(PathBuf::from("/run/user/1000")),
        PathBuf::from("/home/me/.local/share/pod"),
      );

      assert_eq!(resolved, PathBuf::from("/run/user/1000"));
    }
  }

  #[cfg(not(target_os = "windows"))]
  mod stale_recovery {
    use super::*;

    #[test]
    fn a_second_bind_while_a_live_primary_holds_the_socket_is_refused() {
      let (_dir, path) = unique_socket("live");
      let _primary = bind(&path).expect("first bind becomes the primary");

      let second = bind(&path);

      assert!(
        second.is_none(),
        "a second bind while a live primary listens forwards instead of becoming a duplicate"
      );
    }

    #[test]
    fn it_unlinks_a_dead_socket_file_and_becomes_primary() {
      let (_dir, path) = unique_socket("stale");
      std::fs::write(&path, b"orphaned").expect("leave a stale socket file behind");

      let recovered = bind(&path);

      assert!(
        recovered.is_some(),
        "a stale file left by a dead primary is unlinked and the next launch binds"
      );
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
