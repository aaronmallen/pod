#![allow(dead_code)]

use std::{fs, io, path::Path, time::Duration};

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const DEFAULT_STALE_THRESHOLD: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Lease {
  pub db_generation: u64,
  #[serde(deserialize_with = "deserialize_heartbeat", serialize_with = "serialize_heartbeat")]
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

pub fn read_generation(path: &Path) -> u64 {
  fs::read_to_string(path)
    .ok()
    .and_then(|contents| contents.trim().parse().ok())
    .unwrap_or(0)
}

pub fn write_generation(path: &Path, generation: u64) -> io::Result<()> {
  write_atomic(path, generation.to_string().as_bytes())
}

/// Reads the heartbeat from a Unix epoch-millis integer.
///
/// The lease file stores the timestamp as a plain integer rather than RFC 3339 so the format stays language-agnostic
/// and free of chrono's optional serde feature.
fn deserialize_heartbeat<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
  D: Deserializer<'de>,
{
  let millis = i64::deserialize(deserializer)?;
  Utc
    .timestamp_millis_opt(millis)
    .single()
    .ok_or_else(|| serde::de::Error::custom("heartbeat out of range"))
}

fn serialize_heartbeat<S>(heartbeat: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
  S: Serializer,
{
  serializer.serialize_i64(heartbeat.timestamp_millis())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }

  let tmp = path.with_extension("tmp");
  fs::write(&tmp, bytes)?;
  fs::rename(&tmp, path)
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
