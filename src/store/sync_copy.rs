#![allow(dead_code)]

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use chrono::Utc;
use sqlx::{Connection, SqliteConnection};

use crate::store::share_meta::{read_generation, write_generation};

const BACKUP_RETENTION: usize = 3;

const WAL_SIDECARS: [&str; 2] = ["-wal", "-shm"];

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("io error: {0}")]
  Io(#[from] io::Error),
  #[error("database error: {0}")]
  Sqlx(#[from] sqlx::Error),
}

#[derive(Clone, Debug)]
pub struct SyncCopy {
  canonical: PathBuf,
  marker: PathBuf,
  sidecar: PathBuf,
  working_copy: PathBuf,
}

impl SyncCopy {
  pub fn new(canonical: PathBuf, sidecar: PathBuf, working_copy: PathBuf, marker: PathBuf) -> Self {
    Self {
      canonical,
      marker,
      sidecar,
      working_copy,
    }
  }

  pub async fn checkpoint_and_push(&self) -> Result<(), Error> {
    checkpoint(&self.working_copy).await?;

    let share_generation = read_generation(&self.sidecar);
    let local_generation = read_generation(&self.marker);

    // The bytes must land before the generation bumps: a crash between the copy and the sidecar
    // write leaves a generation that understates the bytes (re-pushed next time), never one that
    // overstates them (which would skip a needed pull and silently lose data).
    let next = share_generation.max(local_generation) + 1;
    publish_database(&self.working_copy, &self.canonical, share_generation > local_generation)?;
    write_generation(&self.sidecar, next)?;
    write_generation(&self.marker, next)?;

    Ok(())
  }

  pub fn pull_if_newer(&self) -> Result<bool, Error> {
    if read_generation(&self.sidecar) <= read_generation(&self.marker) {
      return Ok(false);
    }

    // No backup: the single-writer lease guarantees this parked working copy holds no un-pushed
    // writes, so the overwrite can only discard a strict ancestor.
    publish_database(&self.canonical, &self.working_copy, false)?;
    write_generation(&self.marker, read_generation(&self.sidecar))?;

    Ok(true)
  }
}

pub async fn checkpoint_into(source: &Path, destination: &Path) -> Result<(), Error> {
  if let Some(parent) = destination.parent() {
    fs::create_dir_all(parent)?;
  }

  // Stage the database alongside its -wal/-shm sidecars, fold the WAL in, then publish only the
  // self-contained .db — the destination never gains a -wal/-shm trail and the source is untouched.
  let staged = destination.with_extension("checkpoint-tmp");
  fs::copy(source, &staged)?;
  for suffix in WAL_SIDECARS {
    let sidecar = with_suffix(source, suffix);
    if sidecar.exists() {
      fs::copy(&sidecar, with_suffix(&staged, suffix))?;
    }
  }

  let result = checkpoint(&staged).await;
  for suffix in WAL_SIDECARS {
    let _ = fs::remove_file(with_suffix(&staged, suffix));
  }
  if let Err(error) = result {
    let _ = fs::remove_file(&staged);
    return Err(error);
  }

  fs::rename(&staged, destination)?;

  Ok(())
}

/// Deletes the oldest `.backup` siblings of `database`, keeping the newest `keep` by name. The
/// `%Y%m%d-%H%M%S` timestamp sorts lexicographically in chronological order. Best-effort: a
/// failure to list or delete any individual file is ignored.
pub fn prune_backups(database: &Path, keep: usize) {
  let Some(parent) = database.parent() else {
    return;
  };
  let Some(prefix) = database.file_name().map(|name| {
    let mut prefix = name.to_owned();
    prefix.push(".");
    prefix
  }) else {
    return;
  };
  let Ok(entries) = fs::read_dir(parent) else {
    return;
  };

  let mut backups: Vec<PathBuf> = entries
    .filter_map(Result::ok)
    .map(|entry| entry.file_name())
    .filter(|name| {
      let name = name.as_encoded_bytes();
      name.starts_with(prefix.as_encoded_bytes()) && name.ends_with(b".backup")
    })
    .map(|name| parent.join(name))
    .collect();
  backups.sort();

  if backups.len() <= keep {
    return;
  }
  for stale in &backups[..backups.len() - keep] {
    let _ = fs::remove_file(stale);
  }
}

/// The guarded DB-replace primitive every replace site must route through: when `back_up` is set
/// and the destination holds non-empty data, a timestamped `.backup` of it is written (and the set
/// pruned to the newest few) before the overwrite. Routine same-lineage replaces pass `false`.
pub fn publish_database(source: &Path, destination: &Path, back_up: bool) -> io::Result<()> {
  if back_up && is_non_empty(destination) {
    self::back_up(destination)?;
  }
  copy_file(source, destination)?;
  // Any -wal/-shm beside the destination still describes the pre-overwrite database; SQLite would
  // replay them onto the new file on the next open and corrupt it. Sources are already
  // checkpointed into a self-contained .db, so the destination needs no sidecars.
  for suffix in WAL_SIDECARS {
    let _ = fs::remove_file(with_suffix(destination, suffix));
  }

  Ok(())
}

fn back_up(database: &Path) -> io::Result<()> {
  if !database.exists() {
    return Ok(());
  }

  let mut name = database.as_os_str().to_owned();
  name.push(format!(".{}.backup", Utc::now().format("%Y%m%d-%H%M%S")));
  copy_file(database, Path::new(&name))?;
  prune_backups(database, BACKUP_RETENTION);

  Ok(())
}

fn is_non_empty(path: &Path) -> bool {
  fs::metadata(path).is_ok_and(|meta| meta.len() > 0)
}

async fn checkpoint(database: &Path) -> Result<(), Error> {
  let options = sqlx::sqlite::SqliteConnectOptions::new()
    .filename(database)
    .create_if_missing(false);
  let mut connection = SqliteConnection::connect_with(&options).await?;
  sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
    .execute(&mut connection)
    .await?;
  connection.close().await?;

  Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> io::Result<()> {
  if let Some(parent) = destination.parent() {
    fs::create_dir_all(parent)?;
  }

  let tmp = destination.with_extension("tmp");
  fs::copy(source, &tmp)?;
  fs::rename(&tmp, destination)?;

  Ok(())
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
  let mut name = path.as_os_str().to_owned();
  name.push(suffix);
  PathBuf::from(name)
}

#[cfg(test)]
mod tests {
  use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
  use tempfile::TempDir;

  use super::*;

  struct Layout {
    _dir: TempDir,
    canonical: PathBuf,
    marker: PathBuf,
    sidecar: PathBuf,
    working_copy: PathBuf,
  }

  impl Layout {
    fn new() -> Self {
      let dir = tempfile::tempdir().unwrap();
      let share = dir.path().join("share");
      let local = dir.path().join("local");
      fs::create_dir_all(&share).unwrap();
      fs::create_dir_all(&local).unwrap();

      Self {
        canonical: share.join("pod.db"),
        marker: local.join("pod.db.generation"),
        sidecar: share.join("pod.db.generation"),
        working_copy: local.join("pod.db"),
        _dir: dir,
      }
    }

    fn engine(&self) -> SyncCopy {
      SyncCopy::new(
        self.canonical.clone(),
        self.sidecar.clone(),
        self.working_copy.clone(),
        self.marker.clone(),
      )
    }
  }

  async fn seed_wal_database(path: &Path) {
    let options = SqliteConnectOptions::new()
      .filename(path)
      .create_if_missing(true)
      .journal_mode(SqliteJournalMode::Wal);
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    sqlx::query("CREATE TABLE note (body TEXT)")
      .execute(&mut connection)
      .await
      .unwrap();
    sqlx::query("INSERT INTO note (body) VALUES ('hello')")
      .execute(&mut connection)
      .await
      .unwrap();
    connection.close().await.unwrap();
  }

  mod pull_if_newer {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_copies_the_canonical_copy_when_the_share_generation_is_newer() {
      let layout = Layout::new();
      fs::write(&layout.canonical, b"canonical bytes").unwrap();
      write_generation(&layout.sidecar, 5).unwrap();
      write_generation(&layout.marker, 3).unwrap();

      let pulled = layout.engine().pull_if_newer().unwrap();

      assert_eq!(pulled, true);
      assert_eq!(fs::read(&layout.working_copy).unwrap(), b"canonical bytes");
      assert_eq!(read_generation(&layout.marker), 5);
    }

    #[test]
    fn it_transfers_nothing_when_the_generations_are_equal() {
      let layout = Layout::new();
      fs::write(&layout.canonical, b"canonical bytes").unwrap();
      fs::write(&layout.working_copy, b"local bytes").unwrap();
      write_generation(&layout.sidecar, 4).unwrap();
      write_generation(&layout.marker, 4).unwrap();

      let pulled = layout.engine().pull_if_newer().unwrap();

      assert_eq!(pulled, false);
      assert_eq!(fs::read(&layout.working_copy).unwrap(), b"local bytes");
    }

    #[test]
    fn it_transfers_nothing_when_the_local_marker_is_ahead() {
      let layout = Layout::new();
      fs::write(&layout.canonical, b"canonical bytes").unwrap();
      fs::write(&layout.working_copy, b"local bytes").unwrap();
      write_generation(&layout.sidecar, 2).unwrap();
      write_generation(&layout.marker, 9).unwrap();

      let pulled = layout.engine().pull_if_newer().unwrap();

      assert_eq!(pulled, false);
      assert_eq!(fs::read(&layout.working_copy).unwrap(), b"local bytes");
    }

    #[test]
    fn it_does_not_back_up_the_working_copy_when_pulling_a_newer_canonical() {
      let layout = Layout::new();
      fs::write(&layout.canonical, b"share bytes").unwrap();
      fs::write(&layout.working_copy, b"local bytes").unwrap();
      write_generation(&layout.sidecar, 6).unwrap();
      write_generation(&layout.marker, 2).unwrap();

      layout.engine().pull_if_newer().unwrap();

      let backups = fs::read_dir(layout.working_copy.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
        .count();
      assert_eq!(
        backups, 0,
        "the single-writer lease guarantees the parked copy holds no un-pushed writes, so the routine pull never backs up"
      );
      assert_eq!(fs::read(&layout.working_copy).unwrap(), b"share bytes");
    }
  }

  mod checkpoint_and_push {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_checkpoints_the_wal_and_pushes_a_self_contained_database() {
      let layout = Layout::new();
      seed_wal_database(&layout.working_copy).await;

      layout.engine().checkpoint_and_push().await.unwrap();

      let options = SqliteConnectOptions::new().filename(&layout.canonical);
      let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
      let body: String = sqlx::query_scalar("SELECT body FROM note")
        .fetch_one(&mut connection)
        .await
        .unwrap();
      connection.close().await.unwrap();

      assert_eq!(body, "hello");
    }

    #[tokio::test]
    async fn it_bumps_both_generation_markers() {
      let layout = Layout::new();
      seed_wal_database(&layout.working_copy).await;
      write_generation(&layout.sidecar, 7).unwrap();
      write_generation(&layout.marker, 7).unwrap();

      layout.engine().checkpoint_and_push().await.unwrap();

      assert_eq!(read_generation(&layout.sidecar), 8);
      assert_eq!(read_generation(&layout.marker), 8);
    }

    #[tokio::test]
    async fn it_never_copies_the_wal_or_shm_sidecars_to_the_share() {
      let layout = Layout::new();
      seed_wal_database(&layout.working_copy).await;

      layout.engine().checkpoint_and_push().await.unwrap();

      assert!(!layout.canonical.with_extension("db-wal").exists());
      assert!(!layout.canonical.with_extension("db-shm").exists());
    }
  }

  mod checkpoint_into {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_writes_a_self_contained_copy_without_wal_sidecars() {
      let layout = Layout::new();
      let destination = layout.canonical.parent().unwrap().join("consolidated.db");
      seed_wal_database(&layout.working_copy).await;

      checkpoint_into(&layout.working_copy, &destination).await.unwrap();

      let options = SqliteConnectOptions::new().filename(&destination);
      let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
      let body: String = sqlx::query_scalar("SELECT body FROM note")
        .fetch_one(&mut connection)
        .await
        .unwrap();
      connection.close().await.unwrap();

      assert_eq!(body, "hello");
      assert!(!with_suffix(&destination, "-wal").exists());
      assert!(!with_suffix(&destination, "-shm").exists());
    }

    #[tokio::test]
    async fn it_leaves_the_source_in_place() {
      let layout = Layout::new();
      let destination = layout.canonical.parent().unwrap().join("consolidated.db");
      seed_wal_database(&layout.working_copy).await;

      checkpoint_into(&layout.working_copy, &destination).await.unwrap();

      assert!(layout.working_copy.exists(), "the source is not consumed");
    }
  }

  mod divergence {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_backs_up_the_canonical_copy_when_the_share_generation_advanced() {
      let layout = Layout::new();
      seed_wal_database(&layout.working_copy).await;
      fs::write(&layout.canonical, b"diverged canonical").unwrap();
      write_generation(&layout.marker, 3).unwrap();
      write_generation(&layout.sidecar, 8).unwrap();

      layout.engine().checkpoint_and_push().await.unwrap();

      let backup = fs::read_dir(layout.canonical.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
        .expect("a timestamped backup was created");

      assert_eq!(fs::read(backup.path()).unwrap(), b"diverged canonical");
      assert_eq!(read_generation(&layout.sidecar), 9);
    }

    #[tokio::test]
    async fn it_does_not_back_up_a_non_empty_canonical_when_the_generations_are_in_step() {
      let layout = Layout::new();
      seed_wal_database(&layout.working_copy).await;
      fs::write(&layout.canonical, b"in-step canonical").unwrap();
      write_generation(&layout.marker, 4).unwrap();
      write_generation(&layout.sidecar, 4).unwrap();

      layout.engine().checkpoint_and_push().await.unwrap();

      let backups = fs::read_dir(layout.canonical.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
        .count();
      assert_eq!(
        backups, 0,
        "a routine same-lineage push (share generation no newer than local) overwrites a strict ancestor, so no backup is warranted"
      );
    }

    #[tokio::test]
    async fn it_prunes_divergence_backups_to_the_newest_three() {
      let layout = Layout::new();
      seed_wal_database(&layout.working_copy).await;
      fs::write(&layout.canonical, b"diverged canonical").unwrap();
      write_generation(&layout.marker, 3).unwrap();
      write_generation(&layout.sidecar, 8).unwrap();
      for stamp in [
        "20200101-000001",
        "20200101-000002",
        "20200101-000003",
        "20200101-000004",
      ] {
        let mut name = layout.canonical.as_os_str().to_owned();
        name.push(format!(".{stamp}.backup"));
        fs::write(PathBuf::from(name), b"old backup").unwrap();
      }

      layout.engine().checkpoint_and_push().await.unwrap();

      let mut backups: Vec<String> = fs::read_dir(layout.canonical.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".backup"))
        .collect();
      backups.sort();

      assert_eq!(
        backups.len(),
        3,
        "the divergence backup is written and the set is capped at the newest three"
      );
      assert!(
        !backups.iter().any(|name| name.contains("20200101-000001")),
        "the two oldest seeded backups are pruned"
      );
      assert!(!backups.iter().any(|name| name.contains("20200101-000002")));
    }
  }

  mod publish_database {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_backs_up_a_non_empty_destination_before_overwriting_it() {
      let layout = Layout::new();
      // The gen-0-vs-0 data-loss scenario: an empty working copy about to clobber real canonical data.
      fs::write(&layout.canonical, b"real canonical data").unwrap();
      fs::write(&layout.working_copy, b"").unwrap();

      publish_database(&layout.working_copy, &layout.canonical, true).unwrap();

      let backup = fs::read_dir(layout.canonical.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
        .expect("a timestamped backup of the prior canonical exists");
      assert_eq!(fs::read(backup.path()).unwrap(), b"real canonical data");
    }

    #[test]
    fn it_does_not_back_up_when_the_caller_signals_a_routine_replace() {
      let layout = Layout::new();
      fs::write(&layout.canonical, b"real canonical data").unwrap();
      fs::write(&layout.working_copy, b"replacement data").unwrap();

      publish_database(&layout.working_copy, &layout.canonical, false).unwrap();

      let backups = fs::read_dir(layout.canonical.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
        .count();
      assert_eq!(
        backups, 0,
        "a routine replace overwrites a strict ancestor and warrants no backup"
      );
      assert_eq!(fs::read(&layout.canonical).unwrap(), b"replacement data");
    }

    #[test]
    fn it_skips_the_backup_when_the_destination_is_missing_or_empty() {
      let layout = Layout::new();
      fs::write(&layout.working_copy, b"new data").unwrap();
      // Destination absent.
      publish_database(&layout.working_copy, &layout.canonical, true).unwrap();
      // Destination present but empty.
      fs::write(&layout.canonical, b"").unwrap();
      publish_database(&layout.working_copy, &layout.canonical, true).unwrap();

      let backups = fs::read_dir(layout.canonical.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
        .count();
      assert_eq!(backups, 0, "nothing of value is overwritten, so no backup is written");
      assert_eq!(fs::read(&layout.canonical).unwrap(), b"new data");
    }
  }

  mod prune_backups {
    use pretty_assertions::assert_eq;

    use super::*;

    fn seed_backup(database: &Path, stamp: &str) {
      let mut name = database.as_os_str().to_owned();
      name.push(format!(".{stamp}.backup"));
      fs::write(PathBuf::from(name), stamp.as_bytes()).unwrap();
    }

    fn backup_stamps(database: &Path) -> Vec<String> {
      let mut stamps: Vec<String> = fs::read_dir(database.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".backup"))
        .collect();
      stamps.sort();
      stamps
    }

    #[test]
    fn it_keeps_the_newest_three_backups_by_timestamp_name() {
      let layout = Layout::new();
      for stamp in [
        "20260101-000000",
        "20260102-000000",
        "20260103-000000",
        "20260104-000000",
        "20260105-000000",
      ] {
        seed_backup(&layout.canonical, stamp);
      }

      super::super::prune_backups(&layout.canonical, 3);

      let stamps = backup_stamps(&layout.canonical);
      assert_eq!(stamps.len(), 3);
      assert!(
        stamps[0].contains("20260103-000000"),
        "the two oldest backups are pruned"
      );
      assert!(stamps[2].contains("20260105-000000"), "the newest backup is retained");
    }

    #[test]
    fn it_leaves_the_set_untouched_when_within_the_retention_limit() {
      let layout = Layout::new();
      seed_backup(&layout.canonical, "20260101-000000");
      seed_backup(&layout.canonical, "20260102-000000");

      super::super::prune_backups(&layout.canonical, 3);

      assert_eq!(backup_stamps(&layout.canonical).len(), 2);
    }

    #[test]
    fn it_never_touches_non_backup_siblings_of_the_database() {
      let layout = Layout::new();
      fs::write(&layout.canonical, b"live").unwrap();
      write_generation(&layout.sidecar, 5).unwrap();
      fs::write(with_suffix(&layout.canonical, "-wal"), b"wal").unwrap();
      for stamp in [
        "20260101-000000",
        "20260102-000000",
        "20260103-000000",
        "20260104-000000",
      ] {
        seed_backup(&layout.canonical, stamp);
      }

      super::super::prune_backups(&layout.canonical, 3);

      assert!(layout.canonical.exists(), "the live database is untouched");
      assert!(layout.sidecar.exists(), "the generation sidecar is untouched");
      assert!(
        with_suffix(&layout.canonical, "-wal").exists(),
        "the wal sidecar is untouched"
      );
      assert_eq!(backup_stamps(&layout.canonical).len(), 3);
    }
  }
}
