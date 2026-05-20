//! ESI response cache with disk and in-memory backends.

use std::{
  collections::HashMap,
  path::PathBuf,
  sync::Mutex,
  time::{Duration, SystemTime},
};

use bytes::Bytes;
use sha2::{Digest, Sha256};

/// Longest `x-cached-seconds` value across all ESI endpoints (killmails —
/// immutable records). Any cache file older than this can safely be removed.
const MAX_CACHE_AGE: Duration = Duration::from_secs(30_758_400);

/// Selects the storage backend to use when constructing a [`Store`].
#[derive(Debug)]
pub enum CacheType {
  /// Persist cache entries to the given directory on disk.
  Disk(PathBuf),
  /// Keep cache entries in process memory only.
  Memory,
}

/// A unified cache handle that delegates to either a disk or in-memory backend.
#[derive(Clone, Debug)]
pub(crate) enum Store {
  /// Disk-backed variant.
  Disk(DiskStore),
  /// In-memory variant.
  Memory(MemoryStore),
}

impl Store {
  /// Retrieves the cached ETag and body for `url`, if present.
  #[tracing::instrument(skip(self))]
  pub(crate) fn get(&self, url: &str) -> Option<(String, Bytes)> {
    match self {
      Self::Disk(cache) => cache.get(url),
      Self::Memory(cache) => cache.get(url),
    }
  }

  /// Stores `body` along with its `etag` under `url`.
  #[tracing::instrument(skip(self, etag, body))]
  pub(crate) fn insert(&self, url: &str, etag: &str, body: &Bytes) {
    match self {
      Self::Disk(cache) => cache.insert(url, etag, body),
      Self::Memory(cache) => cache.insert(url, etag, body),
    }
  }
}

/// A cache backend that persists entries as files under a directory.
///
/// Each entry is stored as a single file whose name is the SHA-256 hex digest
/// of the URL. The file contains the ETag on the first line, followed by the
/// raw response body.
#[derive(Clone, Debug)]
pub(crate) struct DiskStore {
  path: PathBuf,
}

impl DiskStore {
  /// Creates a new [`DiskStore`] rooted at `path`.
  pub(crate) fn new(path: PathBuf) -> Self {
    Self {
      path,
    }
  }

  /// Reads the cached ETag and body for `url` from disk, returning `None` on
  /// any I/O or parse error.
  #[tracing::instrument(skip(self))]
  pub(crate) fn get(&self, url: &str) -> Option<(String, Bytes)> {
    self.sweep_stale();
    let path = self.file_path(url);
    let content = std::fs::read(&path).ok()?;
    let newline_pos = content.iter().position(|&b| b == b'\n')?;
    let etag = String::from_utf8(content[..newline_pos].to_vec()).ok()?;
    let body = Bytes::from(content[newline_pos + 1..].to_vec());
    Some((etag, body))
  }

  /// Writes `body` and `etag` to disk; logs an error and continues on failure.
  #[tracing::instrument(skip(self, etag, body))]
  pub(crate) fn insert(&self, url: &str, etag: &str, body: &Bytes) {
    if let Err(e) = self.try_store(url, etag, body) {
      tracing::error!("disk cache write failed for {url}: {e}");
    }
    self.sweep_stale();
  }

  /// Returns the file path for a URL by hashing it with SHA-256.
  fn file_path(&self, url: &str) -> PathBuf {
    let hash = Sha256::digest(url.as_bytes());
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    self.path.join(hex)
  }

  /// Removes any cache files whose modification time exceeds [`MAX_CACHE_AGE`].
  fn sweep_stale(&self) {
    let now = SystemTime::now();
    let Ok(entries) = std::fs::read_dir(&self.path) else {
      return;
    };
    for entry in entries.flatten() {
      let Ok(meta) = entry.metadata() else { continue };
      if !meta.is_file() {
        continue;
      }
      let Ok(modified) = meta.modified() else { continue };
      let age = now.duration_since(modified).unwrap_or(Duration::ZERO);
      if age > MAX_CACHE_AGE {
        std::fs::remove_file(entry.path()).ok();
      }
    }
  }

  /// Writes the ETag and body to the cache file, creating the directory if needed.
  fn try_store(&self, url: &str, etag: &str, body: &Bytes) -> std::io::Result<()> {
    std::fs::create_dir_all(&self.path)?;
    let mut content = etag.as_bytes().to_vec();
    content.push(b'\n');
    content.extend_from_slice(body);
    std::fs::write(self.file_path(url), &content)
  }
}

/// A cache backend that stores entries in a `HashMap` guarded by a `Mutex`.
#[derive(Debug, Default)]
pub(crate) struct MemoryStore {
  entries: Mutex<HashMap<String, (String, Bytes, SystemTime)>>,
}

impl MemoryStore {
  /// Returns the cached ETag and body for `url`, or `None` if not present.
  #[tracing::instrument(skip(self))]
  pub(crate) fn get(&self, url: &str) -> Option<(String, Bytes)> {
    self.sweep_stale();
    let map = self.entries.lock().ok()?;
    let (etag, body, _) = map.get(url)?;
    Some((etag.clone(), body.clone()))
  }

  /// Inserts or replaces the entry for `url` with the given `etag` and `body`.
  #[tracing::instrument(skip(self, etag, body))]
  pub(crate) fn insert(&self, url: &str, etag: &str, body: &Bytes) {
    if let Ok(mut map) = self.entries.lock() {
      map.insert(url.to_owned(), (etag.to_owned(), body.to_owned(), SystemTime::now()));
    }
    self.sweep_stale();
  }

  fn sweep_stale(&self) {
    let now = SystemTime::now();
    if let Ok(mut map) = self.entries.lock() {
      map.retain(|_, (_, _, inserted)| now.duration_since(*inserted).unwrap_or(Duration::ZERO) <= MAX_CACHE_AGE);
    }
  }
}

impl Clone for MemoryStore {
  fn clone(&self) -> Self {
    MemoryStore {
      entries: Mutex::new(self.entries.lock().unwrap().clone()),
    }
  }
}
