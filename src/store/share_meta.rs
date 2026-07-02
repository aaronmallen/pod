use std::{fs, io, path::Path, time::Duration};

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const DEFAULT_STALE_THRESHOLD: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Lease {
  pub db_generation: u64,
  #[serde(
    deserialize_with = "deserialize_epoch_millis",
    serialize_with = "serialize_epoch_millis"
  )]
  pub heartbeat: DateTime<Utc>,
  pub hostname: String,
  pub machine_id: String,
  pub pid: u32,
}

impl Lease {
  pub fn read(path: &Path) -> Option<Self> {
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
  }

  pub fn is_stale(&self, threshold: Duration, now: DateTime<Utc>) -> bool {
    match now.signed_duration_since(self.heartbeat).to_std() {
      Ok(elapsed) => elapsed > threshold,
      Err(_) => false,
    }
  }

  pub fn write(&self, path: &Path) -> io::Result<()> {
    let contents = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
    write_atomic(path, contents.as_bytes())
  }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TakeoverRequest {
  pub db_generation: u64,
  #[serde(
    deserialize_with = "deserialize_epoch_millis",
    serialize_with = "serialize_epoch_millis"
  )]
  pub requested_at: DateTime<Utc>,
  pub hostname: String,
  pub machine_id: String,
  pub pid: u32,
}

#[allow(dead_code)]
impl TakeoverRequest {
  pub fn read(path: &Path) -> Option<Self> {
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
  }

  pub fn is_stale(&self, threshold: Duration, now: DateTime<Utc>) -> bool {
    match now.signed_duration_since(self.requested_at).to_std() {
      Ok(elapsed) => elapsed > threshold,
      Err(_) => false,
    }
  }

  pub fn write(&self, path: &Path) -> io::Result<()> {
    let contents = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
    write_atomic(path, contents.as_bytes())
  }
}

pub fn read_generation(path: &Path) -> u64 {
  fs::read_to_string(path)
    .ok()
    .and_then(|contents| contents.trim().parse().ok())
    .unwrap_or(0)
}

pub fn write_generation(path: &Path, generation: u64) -> io::Result<()> {
  write_atomic(path, generation.to_string().as_bytes())
}

/// Reads a timestamp from a Unix epoch-millis integer.
///
/// Share-side metadata stores timestamps as plain integers rather than RFC 3339 so the format stays language-agnostic
/// and free of chrono's optional serde feature.
fn deserialize_epoch_millis<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
  D: Deserializer<'de>,
{
  let millis = i64::deserialize(deserializer)?;
  Utc
    .timestamp_millis_opt(millis)
    .single()
    .ok_or_else(|| serde::de::Error::custom("timestamp out of range"))
}

fn serialize_epoch_millis<S>(timestamp: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
  S: Serializer,
{
  serializer.serialize_i64(timestamp.timestamp_millis())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }

  let tmp = path.with_extension("tmp");
  fs::write(&tmp, bytes)?;
  let Err(rename_error) = fs::rename(&tmp, path) else {
    return Ok(());
  };

  // Some network filesystems (SMB/NFS mounts) reject a rename over an existing file; clearing the
  // destination and retrying, then falling back to a direct write, keeps lease/takeover updates
  // flowing on shares where the atomic path is unavailable.
  let _ = fs::remove_file(path);
  if fs::rename(&tmp, path).is_ok() {
    return Ok(());
  }
  tracing::warn!(
    target: "pod::lifecycle",
    error = %rename_error,
    path = %path.display(),
    "atomic rename failed; falling back to a direct write"
  );
  let result = fs::write(path, bytes);
  let _ = fs::remove_file(&tmp);
  result
}

#[cfg(test)]
mod tests {
  use super::*;

  mod lease {
    use super::*;

    fn sample(heartbeat: DateTime<Utc>) -> Lease {
      Lease {
        db_generation: 7,
        heartbeat,
        hostname: "workstation".to_owned(),
        machine_id: "machine-abc".to_owned(),
        pid: 4242,
      }
    }

    mod is_stale {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_reports_held_exactly_at_the_threshold() {
        let now = Utc::now();
        let lease = sample(now - chrono::Duration::seconds(30));

        assert_eq!(lease.is_stale(DEFAULT_STALE_THRESHOLD, now), false);
      }

      #[test]
      fn it_reports_held_for_a_fresh_heartbeat() {
        let now = Utc::now();
        let lease = sample(now - chrono::Duration::seconds(5));

        assert_eq!(lease.is_stale(DEFAULT_STALE_THRESHOLD, now), false);
      }

      #[test]
      fn it_reports_held_for_a_future_heartbeat() {
        let now = Utc::now();
        let lease = sample(now + chrono::Duration::seconds(10));

        assert_eq!(lease.is_stale(DEFAULT_STALE_THRESHOLD, now), false);
      }

      #[test]
      fn it_reports_stale_past_the_threshold() {
        let now = Utc::now();
        let lease = sample(now - chrono::Duration::seconds(31));

        assert_eq!(lease.is_stale(DEFAULT_STALE_THRESHOLD, now), true);
      }
    }

    mod read {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_none_for_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absent.json");

        assert_eq!(Lease::read(&path), None);
      }

      #[test]
      fn it_returns_none_for_a_truncated_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lease.json");
        let lease = sample(Utc::now());
        let full = serde_json::to_string(&lease).unwrap();
        fs::write(&path, &full[..full.len() / 2]).unwrap();

        assert_eq!(Lease::read(&path), None);
      }

      #[test]
      fn it_returns_none_for_garbage_input() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lease.json");
        fs::write(&path, b"\x00\xff not json at all").unwrap();

        assert_eq!(Lease::read(&path), None);
      }

      #[test]
      fn it_round_trips_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lease.json");
        let lease = sample(Utc.timestamp_millis_opt(1_700_000_000_123).unwrap());

        lease.write(&path).unwrap();

        assert_eq!(Lease::read(&path), Some(lease));
      }
    }
  }

  mod takeover_request {
    use super::*;

    fn sample(requested_at: DateTime<Utc>) -> TakeoverRequest {
      TakeoverRequest {
        db_generation: 7,
        requested_at,
        hostname: "workstation".to_owned(),
        machine_id: "machine-abc".to_owned(),
        pid: 4242,
      }
    }

    mod is_stale {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_reports_active_exactly_at_the_threshold() {
        let now = Utc::now();
        let request = sample(now - chrono::Duration::seconds(30));

        assert_eq!(request.is_stale(DEFAULT_STALE_THRESHOLD, now), false);
      }

      #[test]
      fn it_reports_active_for_a_future_request() {
        let now = Utc::now();
        let request = sample(now + chrono::Duration::seconds(10));

        assert_eq!(request.is_stale(DEFAULT_STALE_THRESHOLD, now), false);
      }

      #[test]
      fn it_reports_stale_past_the_threshold() {
        let now = Utc::now();
        let request = sample(now - chrono::Duration::seconds(31));

        assert_eq!(request.is_stale(DEFAULT_STALE_THRESHOLD, now), true);
      }
    }

    mod read {
      use pretty_assertions::assert_eq;

      use super::*;

      #[test]
      fn it_returns_none_for_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absent.json");

        assert_eq!(TakeoverRequest::read(&path), None);
      }

      #[test]
      fn it_returns_none_for_a_truncated_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("takeover.json");
        let request = sample(Utc::now());
        let full = serde_json::to_string(&request).unwrap();
        fs::write(&path, &full[..full.len() / 2]).unwrap();

        assert_eq!(TakeoverRequest::read(&path), None);
      }

      #[test]
      fn it_returns_none_for_garbage_input() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("takeover.json");
        fs::write(&path, b"\x00\xff not json at all").unwrap();

        assert_eq!(TakeoverRequest::read(&path), None);
      }

      #[test]
      fn it_round_trips_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("takeover.json");
        let request = sample(Utc.timestamp_millis_opt(1_700_000_000_123).unwrap());

        request.write(&path).unwrap();

        assert_eq!(TakeoverRequest::read(&path), Some(request));
      }
    }
  }

  mod write_atomic {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_replaces_an_existing_file() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("lease.json");
      write_atomic(&path, b"first").unwrap();

      write_atomic(&path, b"second").unwrap();

      assert_eq!(fs::read(&path).unwrap(), b"second");
      assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn it_reports_an_error_when_the_destination_cannot_be_replaced() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("lease.json");
      fs::create_dir_all(&path).unwrap();

      assert!(
        write_atomic(&path, b"data").is_err(),
        "a destination that is a directory defeats the rename, the retry, and the direct write"
      );
    }
  }

  mod read_generation {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reads_zero_for_a_missing_file() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("absent");

      assert_eq!(read_generation(&path), 0);
    }

    #[test]
    fn it_reads_zero_for_garbage_input() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("gen");
      fs::write(&path, b"\x00not a number").unwrap();

      assert_eq!(read_generation(&path), 0);
    }

    #[test]
    fn it_round_trips_a_counter() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("gen");

      write_generation(&path, 42).unwrap();

      assert_eq!(read_generation(&path), 42);
    }

    #[test]
    fn it_tolerates_surrounding_whitespace() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join("gen");
      fs::write(&path, "  18\n").unwrap();

      assert_eq!(read_generation(&path), 18);
    }
  }
}
