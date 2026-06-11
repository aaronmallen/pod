use std::{
  fs, io,
  path::{Path, PathBuf},
  time::SystemTime,
};

use chrono::{DateTime, Utc};

use crate::{
  config::{StorageConfig, StorageMode},
  store::{
    lease::{LeaseManager, Outcome},
    share_meta::read_generation,
    sync_copy::{self, SyncCopy},
  },
};

const GENERATION_SUFFIX: &str = ".generation";
const WAL_SUFFIX: &str = "-wal";

#[derive(Clone, Debug)]
pub struct SyncSession {
  engine: SyncCopy,
  lease: LeaseManager,
  marker: PathBuf,
  share: PathBuf,
  sidecar: PathBuf,
  working_copy: PathBuf,
}

impl SyncSession {
  pub fn from_config(storage: &StorageConfig, machine_id: String) -> Option<Self> {
    if storage.storage_mode() != StorageMode::Sync {
      return None;
    }

    let canonical = storage.resolved_database_path();
    let working_copy = storage.resolved_working_copy_path();
    let sidecar = with_suffix(&canonical, GENERATION_SUFFIX);
    let marker = with_suffix(&working_copy, GENERATION_SUFFIX);
    let share = canonical.parent().map(Path::to_path_buf).unwrap_or_default();

    let engine = SyncCopy::new(canonical, sidecar.clone(), working_copy.clone(), marker.clone());
    let host = hostname(&machine_id);
    let lease = LeaseManager::new(machine_id, host, std::process::id(), read_generation(&sidecar));

    Some(Self {
      engine,
      lease,
      marker,
      share,
      sidecar,
      working_copy,
    })
  }

  pub fn acquire(&self, now: DateTime<Utc>) -> io::Result<Outcome> {
    self.lease.acquire(&self.share, now)
  }

  pub async fn checkpoint_and_push(&self) -> Result<(), sync_copy::Error> {
    self.engine.checkpoint_and_push().await
  }

  pub fn has_unsynced_changes(&self) -> bool {
    read_generation(&self.marker) > read_generation(&self.sidecar)
  }

  pub fn heartbeat(&self, now: DateTime<Utc>) -> io::Result<()> {
    self.lease.heartbeat(&self.share, now)
  }

  pub fn is_dirty_since(&self, mark: Option<SystemTime>) -> bool {
    match (self.last_write(), mark) {
      (Some(last_write), Some(mark)) => last_write > mark,
      (Some(_), None) => true,
      (None, _) => false,
    }
  }

  pub fn last_write(&self) -> Option<SystemTime> {
    let database = modified_at(&self.working_copy);
    let wal = modified_at(&with_suffix(&self.working_copy, WAL_SUFFIX));
    match (database, wal) {
      (Some(database), Some(wal)) => Some(database.max(wal)),
      (database, wal) => database.or(wal),
    }
  }

  pub fn pull(&self) -> Result<bool, sync_copy::Error> {
    self.engine.pull_if_newer()
  }

  pub fn release(&self) -> io::Result<()> {
    self.lease.release(&self.share)
  }

  pub fn share_advanced(&self) -> bool {
    read_generation(&self.sidecar) > read_generation(&self.marker)
  }

  /// Reclaims the share, but only once the foreign holder is gone. The re-check goes through
  /// [`SyncSession::acquire`] — whose stale-test declines a still-fresh holder (returning it,
  /// writing nothing) — rather than the unconditional [`LeaseManager::take_over`], so a live writer
  /// is never clobbered. On a successful claim the newer canonical copy is pulled so the working
  /// copy converges before this machine writes again.
  pub fn take_over(&self, now: DateTime<Utc>) -> Result<Outcome, sync_copy::Error> {
    let outcome = self.acquire(now)?;
    if outcome == Outcome::Acquired {
      self.engine.pull_if_newer()?;
    }

    Ok(outcome)
  }
}

fn host_label(os_hostname: &str, machine_id: &str) -> String {
  let trimmed = os_hostname.trim();
  if !trimmed.is_empty() {
    return trimmed.to_owned();
  }

  let short: String = machine_id.chars().take(8).collect();
  format!("machine-{short}")
}

/// Reads the OS hostname via the `gethostname` syscall rather than `HOSTNAME`/`COMPUTERNAME` env
/// vars, which are not exported into GUI-launched processes on macOS and Windows. Falls back to
/// `machine-{first 8 chars of machine_id}` when the syscall returns an empty string.
fn hostname(machine_id: &str) -> String {
  host_label(&gethostname::gethostname().to_string_lossy(), machine_id)
}

fn modified_at(path: &Path) -> Option<SystemTime> {
  fs::metadata(path).and_then(|meta| meta.modified()).ok()
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
  let mut name = path.as_os_str().to_owned();
  name.push(suffix);
  PathBuf::from(name)
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use tempfile::TempDir;

  use super::*;
  use crate::store::share_meta::write_generation;

  struct Fixture {
    _dir: TempDir,
    canonical: PathBuf,
    marker: PathBuf,
    session: SyncSession,
    sidecar: PathBuf,
    working_copy: PathBuf,
  }

  impl Fixture {
    fn new() -> Self {
      let dir = tempfile::tempdir().unwrap();
      let share = dir.path().join("share");
      let cache = dir.path().join("cache");
      fs::create_dir_all(&share).unwrap();
      fs::create_dir_all(&cache).unwrap();

      let mut storage = StorageConfig::default();
      storage.set_db_dir(Some(share.clone()));
      storage.set_cache_dir(Some(cache));
      storage.set_working_copy_dir(Some(dir.path().join("working-copy")));
      storage.set_network(true);

      let session = SyncSession::from_config(&storage, "machine-a".to_owned()).expect("sync mode yields a session");
      let working_copy = storage.resolved_working_copy_path();
      fs::create_dir_all(working_copy.parent().unwrap()).unwrap();

      Self {
        canonical: storage.resolved_database_path(),
        marker: with_suffix(&working_copy, GENERATION_SUFFIX),
        sidecar: with_suffix(&storage.resolved_database_path(), GENERATION_SUFFIX),
        session,
        working_copy,
        _dir: dir,
      }
    }
  }

  mod from_config {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_yields_no_session_in_direct_mode() {
      let dir = tempfile::tempdir().unwrap();
      let mut storage = StorageConfig::default();
      storage.set_db_dir(Some(dir.path().to_path_buf()));

      assert!(SyncSession::from_config(&storage, "machine-a".to_owned()).is_none());
    }

    #[test]
    fn it_seeds_the_lease_db_generation_from_the_share_sidecar() {
      let dir = tempfile::tempdir().unwrap();
      let share = dir.path().join("share");
      fs::create_dir_all(&share).unwrap();
      write_generation(&with_suffix(&share.join("pod.db"), GENERATION_SUFFIX), 12).unwrap();
      let mut storage = StorageConfig::default();
      storage.set_db_dir(Some(share));
      storage.set_cache_dir(Some(dir.path().join("cache")));
      storage.set_working_copy_dir(Some(dir.path().join("working-copy")));
      storage.set_network(true);

      let session = SyncSession::from_config(&storage, "machine-a".to_owned()).unwrap();
      session.acquire(Utc::now()).unwrap();

      let lease = crate::store::share_meta::Lease::read(&LeaseManager::lease_path(&session.share)).unwrap();
      assert_eq!(lease.db_generation, 12);
    }
  }

  mod acquire {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_claims_an_unheld_share() {
      let fixture = Fixture::new();

      assert_eq!(fixture.session.acquire(Utc::now()).unwrap(), Outcome::Acquired);
    }

    #[test]
    fn it_reports_a_fresh_foreign_holder() {
      let fixture = Fixture::new();
      let now = Utc::now();
      LeaseManager::new("machine-b".to_owned(), "host-b".to_owned(), 99, 0)
        .heartbeat(&fixture.session.share, now)
        .unwrap();

      let outcome = fixture.session.acquire(now).unwrap();

      let last_seen = crate::store::share_meta::Lease::read(&LeaseManager::lease_path(&fixture.session.share))
        .unwrap()
        .heartbeat;
      assert_eq!(
        outcome,
        Outcome::HeldBy {
          hostname: "host-b".to_owned(),
          last_seen,
          machine_id: "machine-b".to_owned(),
        }
      );
    }

    #[test]
    fn it_takes_over_a_stale_foreign_lease_from_a_crashed_session() {
      let fixture = Fixture::new();
      let now = Utc::now();
      LeaseManager::new("machine-b".to_owned(), "host-b".to_owned(), 99, 0)
        .heartbeat(&fixture.session.share, now - chrono::Duration::seconds(31))
        .unwrap();

      assert_eq!(fixture.session.acquire(now).unwrap(), Outcome::Acquired);
    }
  }

  mod has_unsynced_changes {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reports_unsynced_when_the_local_marker_is_ahead_of_the_share() {
      let fixture = Fixture::new();
      write_generation(&fixture.marker, 9).unwrap();
      write_generation(&fixture.sidecar, 7).unwrap();

      assert_eq!(fixture.session.has_unsynced_changes(), true);
    }

    #[test]
    fn it_reports_in_sync_when_the_generations_match() {
      let fixture = Fixture::new();
      write_generation(&fixture.marker, 7).unwrap();
      write_generation(&fixture.sidecar, 7).unwrap();

      assert_eq!(fixture.session.has_unsynced_changes(), false);
    }

    #[test]
    fn it_reports_in_sync_when_the_share_is_ahead() {
      let fixture = Fixture::new();
      write_generation(&fixture.marker, 3).unwrap();
      write_generation(&fixture.sidecar, 8).unwrap();

      assert_eq!(fixture.session.has_unsynced_changes(), false);
    }
  }

  mod is_dirty_since {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_clean_when_nothing_has_been_written_since_the_mark() {
      let fixture = Fixture::new();
      fs::write(&fixture.working_copy, b"db").unwrap();
      let mark = fixture.session.last_write();

      assert_eq!(fixture.session.is_dirty_since(mark), false);
    }

    #[test]
    fn it_is_dirty_when_the_wal_was_touched_after_the_mark() {
      let fixture = Fixture::new();
      fs::write(&fixture.working_copy, b"db").unwrap();
      let mark = fixture.session.last_write();

      std::thread::sleep(Duration::from_millis(20));
      fs::write(with_suffix(&fixture.working_copy, WAL_SUFFIX), b"wal").unwrap();

      assert_eq!(fixture.session.is_dirty_since(mark), true);
    }

    #[test]
    fn it_is_dirty_on_the_first_push_when_there_is_no_mark_yet() {
      let fixture = Fixture::new();
      fs::write(&fixture.working_copy, b"db").unwrap();

      assert_eq!(fixture.session.is_dirty_since(None), true);
    }

    #[test]
    fn it_is_clean_when_the_working_copy_does_not_exist() {
      let fixture = Fixture::new();

      assert_eq!(fixture.session.is_dirty_since(None), false);
    }
  }

  mod share_advanced {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_true_when_the_share_generation_outruns_the_local_marker() {
      let fixture = Fixture::new();
      write_generation(&fixture.sidecar, 5).unwrap();
      write_generation(&fixture.marker, 3).unwrap();

      assert_eq!(fixture.session.share_advanced(), true);
    }

    #[test]
    fn it_is_false_when_the_generations_are_in_step() {
      let fixture = Fixture::new();
      write_generation(&fixture.sidecar, 4).unwrap();
      write_generation(&fixture.marker, 4).unwrap();

      assert_eq!(fixture.session.share_advanced(), false);
    }
  }

  mod pull {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_applies_the_newer_share_copy_to_the_working_copy() {
      let fixture = Fixture::new();
      fs::write(&fixture.canonical, b"share data").unwrap();
      write_generation(&fixture.sidecar, 7).unwrap();
      write_generation(&fixture.marker, 2).unwrap();

      let pulled = fixture.session.pull().unwrap();

      assert_eq!(pulled, true);
      assert_eq!(fs::read(&fixture.working_copy).unwrap(), b"share data");
      assert_eq!(read_generation(&fixture.marker), 7);
    }

    #[test]
    fn it_pulls_nothing_when_the_generations_are_in_step() {
      let fixture = Fixture::new();
      fs::write(&fixture.canonical, b"share data").unwrap();
      fs::write(&fixture.working_copy, b"local data").unwrap();
      write_generation(&fixture.sidecar, 3).unwrap();
      write_generation(&fixture.marker, 3).unwrap();

      let pulled = fixture.session.pull().unwrap();

      assert_eq!(pulled, false);
      assert_eq!(fs::read(&fixture.working_copy).unwrap(), b"local data");
    }
  }

  mod take_over {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_declines_a_still_fresh_foreign_lease_without_writing() {
      let fixture = Fixture::new();
      let now = Utc::now();
      LeaseManager::new("machine-b".to_owned(), "host-b".to_owned(), 99, 0)
        .heartbeat(&fixture.session.share, now)
        .unwrap();

      let outcome = fixture.session.take_over(now).unwrap();

      let last_seen = crate::store::share_meta::Lease::read(&LeaseManager::lease_path(&fixture.session.share))
        .unwrap()
        .heartbeat;
      assert_eq!(
        outcome,
        Outcome::HeldBy {
          hostname: "host-b".to_owned(),
          last_seen,
          machine_id: "machine-b".to_owned(),
        }
      );
      assert_eq!(
        crate::store::share_meta::Lease::read(&LeaseManager::lease_path(&fixture.session.share))
          .unwrap()
          .machine_id,
        "machine-b",
        "the fresh foreign holder is left intact"
      );
    }

    #[test]
    fn it_claims_a_stale_foreign_lease() {
      let fixture = Fixture::new();
      let now = Utc::now();
      LeaseManager::new("machine-b".to_owned(), "host-b".to_owned(), 99, 0)
        .heartbeat(&fixture.session.share, now - chrono::Duration::seconds(31))
        .unwrap();

      let outcome = fixture.session.take_over(now).unwrap();

      assert_eq!(outcome, Outcome::Acquired);
      assert_eq!(
        crate::store::share_meta::Lease::read(&LeaseManager::lease_path(&fixture.session.share))
          .unwrap()
          .machine_id,
        "machine-a"
      );
    }

    #[test]
    fn it_pulls_the_newer_canonical_copy_when_claiming() {
      let fixture = Fixture::new();
      fs::write(&fixture.canonical, b"newer canonical").unwrap();
      write_generation(&fixture.sidecar, 9).unwrap();
      write_generation(&fixture.marker, 4).unwrap();

      let outcome = fixture.session.take_over(Utc::now()).unwrap();

      assert_eq!(outcome, Outcome::Acquired);
      assert_eq!(fs::read(&fixture.working_copy).unwrap(), b"newer canonical");
      assert_eq!(read_generation(&fixture.marker), 9);
    }
  }

  mod release {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clears_our_own_lease() {
      let fixture = Fixture::new();
      fixture.session.acquire(Utc::now()).unwrap();

      fixture.session.release().unwrap();

      assert_eq!(
        crate::store::share_meta::Lease::read(&LeaseManager::lease_path(&fixture.session.share)),
        None
      );
    }
  }

  mod hostname {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_resolves_a_non_empty_name() {
      assert!(!super::super::hostname("abcd1234efgh").is_empty());
    }

    #[test]
    fn it_keeps_the_real_os_hostname_when_present() {
      assert_eq!(host_label("studio-mac", "abcd1234efgh"), "studio-mac");
    }

    #[test]
    fn it_trims_surrounding_whitespace_from_the_os_hostname() {
      assert_eq!(host_label("  studio-mac \n", "abcd1234efgh"), "studio-mac");
    }

    #[test]
    fn it_falls_back_to_a_machine_id_label_when_the_os_name_is_blank() {
      assert_eq!(host_label("   ", "abcd1234efgh"), "machine-abcd1234");
    }
  }
}
