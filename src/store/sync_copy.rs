#![allow(dead_code)]

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use chrono::Utc;
use sqlx::{Connection, SqliteConnection};

use crate::store::share_meta::{read_generation, write_generation};

const WAL_SIDECARS: [&str; 2] = ["-wal", "-shm"];

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("io error: {0}")]
  Io(#[from] io::Error),
  #[error("database error: {0}")]
  Sqlx(#[from] sqlx::Error),
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
    publish_database(&self.working_copy, &self.canonical)?;
    write_generation(&self.sidecar, next)?;
    write_generation(&self.marker, next)?;

    Ok(())
  }

  pub fn pull_if_newer(&self) -> Result<bool, Error> {
    if read_generation(&self.sidecar) <= read_generation(&self.marker) {
      return Ok(false);
    }

    copy_file(&self.canonical, &self.working_copy)?;
    write_generation(&self.marker, read_generation(&self.sidecar))?;

    Ok(true)
  }
}

/// The guarded DB-replace primitive every replace site must route through: never overwrites a
/// non-empty destination without first writing a timestamped `.backup` of it.
pub fn publish_database(source: &Path, destination: &Path) -> io::Result<()> {
  if is_non_empty(destination) {
    back_up(destination)?;
  }
  copy_file(source, destination)
}

fn back_up(database: &Path) -> io::Result<()> {
  if !database.exists() {
    return Ok(());
  }

  let mut name = database.as_os_str().to_owned();
  name.push(format!(".{}.backup", Utc::now().format("%Y%m%d-%H%M%S")));
  copy_file(database, Path::new(&name))
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
    async fn it_backs_up_a_non_empty_canonical_even_when_the_generations_are_in_step() {
      let layout = Layout::new();
      seed_wal_database(&layout.working_copy).await;
      fs::write(&layout.canonical, b"in-step canonical").unwrap();
      write_generation(&layout.marker, 4).unwrap();
      write_generation(&layout.sidecar, 4).unwrap();

      layout.engine().checkpoint_and_push().await.unwrap();

      let backup = fs::read_dir(layout.canonical.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
        .expect("the prior canonical is backed up before the in-step push overwrites it");

      assert_eq!(
        fs::read(backup.path()).unwrap(),
        b"in-step canonical",
        "the data-loss hole is closed: the real canonical survives as a backup"
      );
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

      publish_database(&layout.working_copy, &layout.canonical).unwrap();

      let backup = fs::read_dir(layout.canonical.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
        .expect("a timestamped backup of the prior canonical exists");
      assert_eq!(fs::read(backup.path()).unwrap(), b"real canonical data");
    }

    #[test]
    fn it_skips_the_backup_when_the_destination_is_missing_or_empty() {
      let layout = Layout::new();
      fs::write(&layout.working_copy, b"new data").unwrap();
      // Destination absent.
      publish_database(&layout.working_copy, &layout.canonical).unwrap();
      // Destination present but empty.
      fs::write(&layout.canonical, b"").unwrap();
      publish_database(&layout.working_copy, &layout.canonical).unwrap();

      let backups = fs::read_dir(layout.canonical.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
        .count();
      assert_eq!(backups, 0, "nothing of value is overwritten, so no backup is written");
      assert_eq!(fs::read(&layout.canonical).unwrap(), b"new data");
    }
  }
}
