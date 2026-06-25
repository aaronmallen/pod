use std::{
  io,
  path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};

use crate::{
  config::{StorageConfig, StorageMode},
  store::{
    bootstrap,
    lease::Outcome,
    share_meta::{read_generation, write_generation},
    sync_copy::{self, publish_database},
    sync_session::SyncSession,
  },
};

const GENERATION_SUFFIX: &str = ".generation";

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("io error: {0}")]
  Io(#[from] io::Error),
  #[error("the sync share is held by {hostname} ({machine_id}); import is refused until that machine releases it")]
  LeaseHeld { hostname: String, machine_id: String },
  #[error("bootstrap error: {0}")]
  Bootstrap(#[from] bootstrap::Error),
  #[error("sync error: {0}")]
  Sync(#[from] sync_copy::Error),
}

/// Atomically replaces the live database with the archived snapshot staged at `temp_db`, backing up
/// the prior state first. In Direct mode the canonical file is replaced in place. In Sync mode the
/// lease must be held: the archive lands on the shared canonical and the generation is bumped past
/// the local marker so the next launch's `pull_if_newer` re-seeds the working copy from canonical,
/// leaving the exit `checkpoint_and_push` unable to clobber the restore with a stale working copy.
#[allow(dead_code)]
pub fn restore(storage: &StorageConfig, machine_id: String, temp_db: &Path, now: DateTime<Utc>) -> Result<(), Error> {
  match storage.storage_mode() {
    StorageMode::Direct => restore_direct(storage, temp_db),
    StorageMode::Sync => restore_sync(storage, machine_id, temp_db, now),
  }
}

fn restore_direct(storage: &StorageConfig, temp_db: &Path) -> Result<(), Error> {
  let target = bootstrap::resolve_local_path(storage)?;
  publish_database(temp_db, &target, true)?;

  Ok(())
}

fn restore_sync(storage: &StorageConfig, machine_id: String, temp_db: &Path, now: DateTime<Utc>) -> Result<(), Error> {
  let session = SyncSession::from_config(storage, machine_id)
    .ok_or_else(|| io::Error::other("sync session unavailable for a non-sync storage config"))?;

  // Hold the lease before touching the shared canonical: a foreign holder may be actively writing it.
  if let Outcome::HeldBy {
    hostname,
    machine_id,
    ..
  } = session.acquire(now)?
  {
    return Err(Error::LeaseHeld {
      hostname,
      machine_id,
    });
  }

  let canonical = storage.resolved_database_path();
  let sidecar = with_suffix(&canonical, GENERATION_SUFFIX);
  let marker = with_suffix(&storage.resolved_working_copy_path(), GENERATION_SUFFIX);

  // Bytes land before the generation bumps (the canonical -> sidecar ordering): a crash after the
  // copy but before the sidecar write leaves a generation that understates the canonical, which the
  // next push re-establishes, never one that overstates it and skips a needed re-seed.
  publish_database(temp_db, &canonical, true)?;

  // Advance the share sidecar strictly past the local marker without touching the marker, so the
  // next boot's pull_if_newer (sidecar > marker) re-seeds the working copy from the restored
  // canonical and the exit checkpoint_and_push cannot push the stale working copy over it.
  let next = read_generation(&sidecar).max(read_generation(&marker)) + 1;
  write_generation(&sidecar, next)?;

  Ok(())
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
  let mut name = path.as_os_str().to_owned();
  name.push(suffix);
  PathBuf::from(name)
}

#[cfg(test)]
mod tests {
  use std::fs;

  use tempfile::TempDir;

  use super::*;
  use crate::store::sync_copy::SyncCopy;

  fn sync_storage(dir: &TempDir) -> StorageConfig {
    let mut storage = StorageConfig::default();
    storage.set_db_dir(Some(dir.path().join("share")));
    storage.set_cache_dir(Some(dir.path().join("cache")));
    storage.set_working_copy_dir(Some(dir.path().join("working-copy")));
    storage.set_network(true);
    storage
  }

  fn staged_archive(dir: &TempDir, bytes: &[u8]) -> PathBuf {
    let path = dir.path().join("staged.db");
    fs::write(&path, bytes).unwrap();
    path
  }

  mod restore_direct {
    use pretty_assertions::assert_eq;

    use super::*;

    fn direct_storage(dir: &TempDir) -> StorageConfig {
      let mut storage = StorageConfig::default();
      storage.set_db_dir(Some(dir.path().join("data")));
      storage
    }

    #[test]
    fn it_replaces_the_canonical_database_with_the_archive() {
      let dir = tempfile::tempdir().unwrap();
      let storage = direct_storage(&dir);
      fs::create_dir_all(storage.resolved_db_dir()).unwrap();
      fs::write(storage.resolved_database_path(), b"old live data").unwrap();
      let temp_db = staged_archive(&dir, b"restored archive data");

      restore(&storage, "machine-a".to_owned(), &temp_db, Utc::now()).unwrap();

      assert_eq!(
        fs::read(storage.resolved_database_path()).unwrap(),
        b"restored archive data"
      );
    }

    #[test]
    fn it_backs_up_the_prior_database_before_overwriting_it() {
      let dir = tempfile::tempdir().unwrap();
      let storage = direct_storage(&dir);
      fs::create_dir_all(storage.resolved_db_dir()).unwrap();
      fs::write(storage.resolved_database_path(), b"old live data").unwrap();
      let temp_db = staged_archive(&dir, b"restored archive data");

      restore(&storage, "machine-a".to_owned(), &temp_db, Utc::now()).unwrap();

      let backup = fs::read_dir(storage.resolved_db_dir())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
        .expect("a timestamped backup of the prior database exists");
      assert_eq!(fs::read(backup.path()).unwrap(), b"old live data");
    }

    #[test]
    fn it_removes_stale_wal_sidecars_beside_the_restored_database() {
      let dir = tempfile::tempdir().unwrap();
      let storage = direct_storage(&dir);
      fs::create_dir_all(storage.resolved_db_dir()).unwrap();
      let canonical = storage.resolved_database_path();
      fs::write(&canonical, b"old live data").unwrap();
      fs::write(with_suffix(&canonical, "-wal"), b"stale wal").unwrap();
      fs::write(with_suffix(&canonical, "-shm"), b"stale shm").unwrap();
      let temp_db = staged_archive(&dir, b"restored archive data");

      restore(&storage, "machine-a".to_owned(), &temp_db, Utc::now()).unwrap();

      assert!(!with_suffix(&canonical, "-wal").exists());
      assert!(!with_suffix(&canonical, "-shm").exists());
    }
  }

  mod restore_sync {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::lease::LeaseManager;

    fn canonical(storage: &StorageConfig) -> PathBuf {
      storage.resolved_database_path()
    }

    fn sidecar(storage: &StorageConfig) -> PathBuf {
      with_suffix(&canonical(storage), GENERATION_SUFFIX)
    }

    fn marker(storage: &StorageConfig) -> PathBuf {
      with_suffix(&storage.resolved_working_copy_path(), GENERATION_SUFFIX)
    }

    #[test]
    fn it_writes_the_archive_to_the_canonical_path() {
      let dir = tempfile::tempdir().unwrap();
      let storage = sync_storage(&dir);
      fs::create_dir_all(storage.resolved_db_dir()).unwrap();
      fs::write(canonical(&storage), b"old canonical data").unwrap();
      let temp_db = staged_archive(&dir, b"restored archive data");

      restore(&storage, "machine-a".to_owned(), &temp_db, Utc::now()).unwrap();

      assert_eq!(fs::read(canonical(&storage)).unwrap(), b"restored archive data");
    }

    #[test]
    fn it_backs_up_the_prior_canonical_before_overwriting_it() {
      let dir = tempfile::tempdir().unwrap();
      let storage = sync_storage(&dir);
      fs::create_dir_all(storage.resolved_db_dir()).unwrap();
      fs::write(canonical(&storage), b"old canonical data").unwrap();
      let temp_db = staged_archive(&dir, b"restored archive data");

      restore(&storage, "machine-a".to_owned(), &temp_db, Utc::now()).unwrap();

      let backup = fs::read_dir(storage.resolved_db_dir())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
        .expect("a timestamped backup of the prior canonical exists");
      assert_eq!(fs::read(backup.path()).unwrap(), b"old canonical data");
    }

    #[test]
    fn it_bumps_the_share_sidecar_past_the_local_marker() {
      let dir = tempfile::tempdir().unwrap();
      let storage = sync_storage(&dir);
      fs::create_dir_all(storage.resolved_db_dir()).unwrap();
      write_generation(&sidecar(&storage), 4).unwrap();
      write_generation(&marker(&storage), 7).unwrap();
      let temp_db = staged_archive(&dir, b"restored archive data");

      restore(&storage, "machine-a".to_owned(), &temp_db, Utc::now()).unwrap();

      assert!(
        read_generation(&sidecar(&storage)) > read_generation(&marker(&storage)),
        "the share sidecar outruns the local marker so the working copy re-seeds on next boot"
      );
      assert_eq!(
        read_generation(&marker(&storage)),
        7,
        "the local marker is left behind so pull_if_newer detects the newer canonical"
      );
    }

    #[test]
    fn it_leaves_a_subsequent_pull_re_seeding_the_working_copy_from_canonical() {
      let dir = tempfile::tempdir().unwrap();
      let storage = sync_storage(&dir);
      fs::create_dir_all(storage.resolved_db_dir()).unwrap();
      let working_copy = storage.resolved_working_copy_path();
      fs::create_dir_all(working_copy.parent().unwrap()).unwrap();
      fs::write(&working_copy, b"stale working copy").unwrap();
      let temp_db = staged_archive(&dir, b"restored archive data");

      restore(&storage, "machine-a".to_owned(), &temp_db, Utc::now()).unwrap();

      let engine = SyncCopy::new(
        canonical(&storage),
        sidecar(&storage),
        working_copy.clone(),
        marker(&storage),
      );
      assert_eq!(engine.pull_if_newer().unwrap(), true);
      assert_eq!(
        fs::read(&working_copy).unwrap(),
        b"restored archive data",
        "the working copy re-seeds from the restored canonical"
      );
    }

    #[test]
    fn it_refuses_when_another_machine_holds_the_lease() {
      let dir = tempfile::tempdir().unwrap();
      let storage = sync_storage(&dir);
      let share = storage.resolved_db_dir();
      fs::create_dir_all(&share).unwrap();
      fs::write(canonical(&storage), b"old canonical data").unwrap();
      let now = Utc::now();
      LeaseManager::new("machine-b".to_owned(), "host-b".to_owned(), 99, 0)
        .heartbeat(&share, now)
        .unwrap();
      let temp_db = staged_archive(&dir, b"restored archive data");

      let result = restore(&storage, "machine-a".to_owned(), &temp_db, now);

      assert!(matches!(
        result,
        Err(Error::LeaseHeld {
          ref machine_id, ..
        }) if machine_id == "machine-b"
      ));
      assert_eq!(
        fs::read(canonical(&storage)).unwrap(),
        b"old canonical data",
        "the canonical is untouched when the lease is held elsewhere"
      );
    }
  }
}
