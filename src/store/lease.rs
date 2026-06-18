use std::{
  fs, io,
  path::{Path, PathBuf},
  time::Duration,
};

use chrono::{DateTime, Utc};

use crate::store::share_meta::{DEFAULT_STALE_THRESHOLD, Lease};

pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
pub const LEASE_FILE_NAME: &str = "lease.json";
pub const STALE_THRESHOLD: Duration = DEFAULT_STALE_THRESHOLD;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
  Acquired,
  HeldBy {
    hostname: String,
    last_seen: DateTime<Utc>,
    machine_id: String,
  },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeaseManager {
  db_generation: u64,
  hostname: String,
  machine_id: String,
  pid: u32,
}

impl LeaseManager {
  pub fn new(machine_id: String, hostname: String, pid: u32, db_generation: u64) -> Self {
    Self {
      db_generation,
      hostname,
      machine_id,
      pid,
    }
  }

  pub fn lease_path(share: &Path) -> PathBuf {
    share.join(LEASE_FILE_NAME)
  }

  pub fn acquire(&self, share: &Path, now: DateTime<Utc>) -> io::Result<Outcome> {
    let path = Self::lease_path(share);

    if let Some(existing) = Lease::read(&path)
      && existing.machine_id != self.machine_id
      && !existing.is_stale(STALE_THRESHOLD, now)
    {
      return Ok(Outcome::HeldBy {
        hostname: existing.hostname,
        last_seen: existing.heartbeat,
        machine_id: existing.machine_id,
      });
    }

    self.write(&path, now)?;

    Ok(Outcome::Acquired)
  }

  pub fn heartbeat(&self, share: &Path, now: DateTime<Utc>) -> io::Result<()> {
    self.write(&Self::lease_path(share), now)
  }

  pub fn release(&self, share: &Path) -> io::Result<()> {
    let path = Self::lease_path(share);

    match Lease::read(&path) {
      Some(existing) if existing.machine_id != self.machine_id => Ok(()),
      _ => match fs::remove_file(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
      },
    }
  }

  pub fn take_over(&self, share: &Path, now: DateTime<Utc>) -> io::Result<()> {
    self.write(&Self::lease_path(share), now)
  }

  fn lease(&self, heartbeat: DateTime<Utc>) -> Lease {
    Lease {
      db_generation: self.db_generation,
      heartbeat,
      hostname: self.hostname.clone(),
      machine_id: self.machine_id.clone(),
      pid: self.pid,
    }
  }

  fn write(&self, path: &Path, now: DateTime<Utc>) -> io::Result<()> {
    self.lease(now).write(path)
  }
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone;

  use super::*;

  fn at(millis: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(millis).unwrap()
  }

  fn manager(machine_id: &str) -> LeaseManager {
    LeaseManager::new(machine_id.to_owned(), format!("host-{machine_id}"), 4242, 7)
  }

  mod acquire {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_claims_an_unheld_share() {
      let dir = tempfile::tempdir().unwrap();
      let now = Utc::now();
      let manager = manager("machine-a");

      let outcome = manager.acquire(dir.path(), now).unwrap();

      assert_eq!(outcome, Outcome::Acquired);
      assert_eq!(
        Lease::read(&LeaseManager::lease_path(dir.path())).unwrap().machine_id,
        "machine-a"
      );
    }

    #[test]
    fn it_refreshes_our_own_fresh_lease() {
      let dir = tempfile::tempdir().unwrap();
      let earlier = at(1_700_000_000_000);
      let now = at(1_700_000_005_000);
      let manager = manager("machine-a");
      manager.write(&LeaseManager::lease_path(dir.path()), earlier).unwrap();

      let outcome = manager.acquire(dir.path(), now).unwrap();

      assert_eq!(outcome, Outcome::Acquired);
      assert_eq!(
        Lease::read(&LeaseManager::lease_path(dir.path())).unwrap().heartbeat,
        now
      );
    }

    #[test]
    fn it_reports_held_by_a_fresh_foreign_lease() {
      let dir = tempfile::tempdir().unwrap();
      let heartbeat = at(1_700_000_005_000);
      let now = at(1_700_000_010_000);
      manager("machine-b")
        .write(&LeaseManager::lease_path(dir.path()), heartbeat)
        .unwrap();

      let outcome = manager("machine-a").acquire(dir.path(), now).unwrap();

      assert_eq!(
        outcome,
        Outcome::HeldBy {
          hostname: "host-machine-b".to_owned(),
          last_seen: heartbeat,
          machine_id: "machine-b".to_owned(),
        }
      );
      assert_eq!(
        Lease::read(&LeaseManager::lease_path(dir.path())).unwrap().machine_id,
        "machine-b"
      );
    }

    #[test]
    fn it_takes_over_a_stale_foreign_lease() {
      let dir = tempfile::tempdir().unwrap();
      let now = Utc::now();
      manager("machine-b")
        .write(
          &LeaseManager::lease_path(dir.path()),
          now - chrono::Duration::seconds(31),
        )
        .unwrap();

      let outcome = manager("machine-a").acquire(dir.path(), now).unwrap();

      assert_eq!(outcome, Outcome::Acquired);
      assert_eq!(
        Lease::read(&LeaseManager::lease_path(dir.path())).unwrap().machine_id,
        "machine-a"
      );
    }
  }

  mod heartbeat {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_advances_the_stored_timestamp() {
      let dir = tempfile::tempdir().unwrap();
      let acquired_at = at(1_700_000_000_000);
      let beat_at = at(1_700_000_008_000);
      let manager = manager("machine-a");
      manager.acquire(dir.path(), acquired_at).unwrap();

      manager.heartbeat(dir.path(), beat_at).unwrap();

      assert_eq!(
        Lease::read(&LeaseManager::lease_path(dir.path())).unwrap().heartbeat,
        beat_at
      );
    }
  }

  mod release {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_a_no_op_when_no_lease_exists() {
      let dir = tempfile::tempdir().unwrap();

      manager("machine-a").release(dir.path()).unwrap();

      assert_eq!(Lease::read(&LeaseManager::lease_path(dir.path())), None);
    }

    #[test]
    fn it_leaves_a_foreign_lease_intact() {
      let dir = tempfile::tempdir().unwrap();
      let now = Utc::now();
      manager("machine-b").acquire(dir.path(), now).unwrap();

      manager("machine-a").release(dir.path()).unwrap();

      assert_eq!(
        Lease::read(&LeaseManager::lease_path(dir.path())).unwrap().machine_id,
        "machine-b"
      );
    }

    #[test]
    fn it_removes_our_lease_and_allows_reacquire() {
      let dir = tempfile::tempdir().unwrap();
      let now = Utc::now();
      let holder = manager("machine-a");
      holder.acquire(dir.path(), now).unwrap();

      holder.release(dir.path()).unwrap();

      assert_eq!(Lease::read(&LeaseManager::lease_path(dir.path())), None);
      assert_eq!(
        manager("machine-b").acquire(dir.path(), now).unwrap(),
        Outcome::Acquired
      );
    }
  }

  mod take_over {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_claims_regardless_of_a_fresh_foreign_lease() {
      let dir = tempfile::tempdir().unwrap();
      let now = Utc::now();
      manager("machine-b")
        .write(
          &LeaseManager::lease_path(dir.path()),
          now - chrono::Duration::seconds(5),
        )
        .unwrap();

      manager("machine-a").take_over(dir.path(), now).unwrap();

      assert_eq!(
        Lease::read(&LeaseManager::lease_path(dir.path())).unwrap().machine_id,
        "machine-a"
      );
    }
  }
}
