#![allow(dead_code)]

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use chrono::Utc;
use sqlx::{Connection, SqliteConnection};

use crate::store::share_meta::{read_generation, write_generation};

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
    if share_generation > local_generation {
      back_up(&self.canonical)?;
    }

    // The bytes must land before the generation bumps: a crash between the copy and the sidecar
    // write leaves a generation that understates the bytes (re-pushed next time), never one that
    // overstates them (which would skip a needed pull and silently lose data).
    let next = share_generation.max(local_generation) + 1;
    copy_file(&self.working_copy, &self.canonical)?;
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

fn back_up(canonical: &Path) -> io::Result<()> {
  if !canonical.exists() {
    return Ok(());
  }

  let mut name = canonical.as_os_str().to_owned();
  name.push(format!(".{}.backup", Utc::now().format("%Y%m%d-%H%M%S")));
  copy_file(canonical, Path::new(&name))
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
    async fn it_does_not_back_up_when_the_share_generation_is_in_step() {
      let layout = Layout::new();
      seed_wal_database(&layout.working_copy).await;
      fs::write(&layout.canonical, b"in-step canonical").unwrap();
      write_generation(&layout.marker, 4).unwrap();
      write_generation(&layout.sidecar, 4).unwrap();

      layout.engine().checkpoint_and_push().await.unwrap();

      let backup = fs::read_dir(layout.canonical.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().ends_with(".backup"));

      assert!(backup.is_none());
    }
  }
}
