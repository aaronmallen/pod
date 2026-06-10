use std::{
  fs, io,
  path::{Path, PathBuf},
};

use crate::{
  config::{StorageConfig, StorageMode},
  store::{
    share_meta::{read_generation, write_generation},
    sync_copy::checkpoint_into,
  },
};

const EXDEV: i32 = 18;
const GENERATION_SUFFIX: &str = ".generation";
const WAL_SIDECARS: [&str; 2] = ["-wal", "-shm"];

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("io error: {0}")]
  Io(#[from] io::Error),
  #[error("sync error: {0}")]
  Sync(#[from] crate::store::sync_copy::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transition {
  DirectToDirect,
  DirectToSync,
  SyncToDirect,
  SyncToSync,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Layout {
  canonical: PathBuf,
  marker: PathBuf,
  sidecar: PathBuf,
  working_copy: PathBuf,
}

impl Layout {
  fn from_config(storage: &StorageConfig) -> Self {
    let canonical = storage.resolved_database_path();
    let working_copy = storage.resolved_working_copy_path();
    Self {
      sidecar: generation_marker(&canonical),
      marker: generation_marker(&working_copy),
      canonical,
      working_copy,
    }
  }
}

pub fn transition(old_mode: StorageMode, new_mode: StorageMode) -> Transition {
  match (old_mode, new_mode) {
    (StorageMode::Direct, StorageMode::Direct) => Transition::DirectToDirect,
    (StorageMode::Direct, StorageMode::Sync) => Transition::DirectToSync,
    (StorageMode::Sync, StorageMode::Direct) => Transition::SyncToDirect,
    (StorageMode::Sync, StorageMode::Sync) => Transition::SyncToSync,
  }
}

pub async fn migrate(
  old: &StorageConfig,
  new: &StorageConfig,
  old_mode: StorageMode,
  new_mode: StorageMode,
) -> Result<(), Error> {
  let from = Layout::from_config(old);
  let to = Layout::from_config(new);

  match transition(old_mode, new_mode) {
    Transition::DirectToDirect => migrate_direct_to_direct(&from, &to),
    Transition::DirectToSync => migrate_direct_to_sync(&from, &to),
    Transition::SyncToDirect => migrate_sync_to_direct(&from, &to).await,
    Transition::SyncToSync => migrate_sync_to_sync(&from, &to),
  }
}

fn migrate_direct_to_direct(from: &Layout, to: &Layout) -> Result<(), Error> {
  if paths_equal(&from.canonical, &to.canonical) {
    return Ok(());
  }
  if !from.canonical.exists() {
    return Ok(());
  }

  ensure_parent(&to.canonical)?;
  // Carry the -wal/-shm sidecars first so SQLite never discards an orphaned WAL at the old path.
  for suffix in WAL_SIDECARS {
    move_sidecar(&from.canonical, &to.canonical, suffix)?;
  }
  move_file(&from.canonical, &to.canonical)?;

  Ok(())
}

fn migrate_direct_to_sync(from: &Layout, to: &Layout) -> Result<(), Error> {
  if !from.canonical.exists() {
    // A fresh install with no database yet: the working copy is seeded at first launch.
    return Ok(());
  }

  // Stage the live database onto the share before touching the source: a failure here leaves the
  // old Direct layout fully usable. When the configured path is unchanged (an in-place sync toggle)
  // the canonical file already sits where it belongs, so only the sidecar and working copy are new.
  ensure_parent(&to.canonical)?;
  if !paths_equal(&from.canonical, &to.canonical) {
    copy_file(&from.canonical, &to.canonical)?;
  }
  let next = read_generation(&to.sidecar).max(read_generation(&from.sidecar)) + 1;
  write_generation(&to.sidecar, next)?;

  // Seed the local working copy and mark it in step with the share so the first launch performs no
  // redundant pull. Roll back every artifact this migration created if seeding fails, so the old
  // Direct layout stays fully usable.
  if let Err(error) = seed_working_copy(&from.canonical, to, next) {
    let _ = fs::remove_file(&to.working_copy);
    let _ = fs::remove_file(&to.marker);
    let _ = fs::remove_file(&to.sidecar);
    if !paths_equal(&from.canonical, &to.canonical) {
      let _ = fs::remove_file(&to.canonical);
    }
    return Err(error);
  }

  // The old Direct database is now redundant; only tear it down when it is a distinct file from the
  // new canonical copy.
  if !paths_equal(&from.canonical, &to.canonical) {
    remove_database_family(&from.canonical);
  }

  Ok(())
}

async fn migrate_sync_to_direct(from: &Layout, to: &Layout) -> Result<(), Error> {
  // The working copy holds the freshest data (it may carry an uncheckpointed WAL). Consolidate it
  // into a single self-contained file at the new local location, preferring it over the share copy.
  let source = if from.working_copy.exists() {
    &from.working_copy
  } else if from.canonical.exists() {
    &from.canonical
  } else {
    return Ok(());
  };

  ensure_parent(&to.canonical)?;
  // checkpoint_into folds the WAL into a fresh standalone copy, so no -wal/-shm trails the result.
  checkpoint_into(source, &to.canonical).await?;

  // Only now tear down the old Sync arrangement, and never the consolidated file we just wrote.
  if !paths_equal(&from.working_copy, &to.canonical) {
    remove_database_family(&from.working_copy);
  }
  if paths_equal(&from.canonical, &to.canonical) {
    // The canonical path is reused in place (a sync toggle), so drop only the now-meaningless
    // generation sidecar rather than the freshly written database itself.
    let _ = fs::remove_file(&from.sidecar);
  } else {
    remove_database_family(&from.canonical);
  }

  Ok(())
}

fn migrate_sync_to_sync(from: &Layout, to: &Layout) -> Result<(), Error> {
  if paths_equal(&from.canonical, &to.canonical) {
    return Ok(());
  }

  // Carry the canonical copy plus its generation sidecar to the new share, then the working copy and
  // its marker follow so the next launch sees them in step.
  if from.canonical.exists() {
    ensure_parent(&to.canonical)?;
    copy_file(&from.canonical, &to.canonical)?;
    if from.sidecar.exists() {
      copy_file(&from.sidecar, &to.sidecar)?;
    }
    remove_database_family(&from.canonical);
  }

  if !paths_equal(&from.working_copy, &to.working_copy) && from.working_copy.exists() {
    ensure_parent(&to.working_copy)?;
    for suffix in WAL_SIDECARS {
      move_sidecar(&from.working_copy, &to.working_copy, suffix)?;
    }
    move_file(&from.working_copy, &to.working_copy)?;
    if from.marker.exists() {
      move_file(&from.marker, &to.marker)?;
    }
  }

  Ok(())
}

fn seed_working_copy(source: &Path, to: &Layout, generation: u64) -> Result<(), Error> {
  ensure_parent(&to.working_copy)?;
  copy_file(source, &to.working_copy)?;
  write_generation(&to.marker, generation)?;
  Ok(())
}

fn ensure_parent(path: &Path) -> io::Result<()> {
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  Ok(())
}

fn generation_marker(database: &Path) -> PathBuf {
  with_suffix(database, GENERATION_SUFFIX)
}

fn copy_file(source: &Path, destination: &Path) -> io::Result<()> {
  ensure_parent(destination)?;
  let tmp = destination.with_extension("migrate-tmp");
  fs::copy(source, &tmp)?;
  fs::rename(&tmp, destination)
}

fn move_file(from: &Path, to: &Path) -> io::Result<()> {
  match fs::rename(from, to) {
    Ok(()) => return Ok(()),
    Err(error) if error.raw_os_error() == Some(EXDEV) => {}
    Err(error) => return Err(error),
  }

  fs::copy(from, to)?;
  fs::remove_file(from)
}

fn move_sidecar(from: &Path, to: &Path, suffix: &str) -> io::Result<()> {
  let source = with_suffix(from, suffix);
  if source.exists() {
    move_file(&source, &with_suffix(to, suffix))?;
  }
  Ok(())
}

fn remove_database_family(database: &Path) {
  let _ = fs::remove_file(database);
  let _ = fs::remove_file(generation_marker(database));
  for suffix in WAL_SIDECARS {
    let _ = fs::remove_file(with_suffix(database, suffix));
  }
}

fn paths_equal(a: &Path, b: &Path) -> bool {
  match (fs::canonicalize(a), fs::canonicalize(b)) {
    (Ok(a), Ok(b)) => a == b,
    _ => a == b,
  }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
  let mut name = path.as_os_str().to_owned();
  name.push(suffix);
  PathBuf::from(name)
}

#[cfg(test)]
mod tests {
  use sqlx::{
    Connection, SqliteConnection,
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
  };
  use tempfile::{TempDir, tempdir};

  use super::*;

  fn config(db_dir: &Path, cache_dir: &Path, network: bool) -> StorageConfig {
    let mut storage = StorageConfig::default();
    storage.set_db_dir(Some(db_dir.to_path_buf()));
    storage.set_cache_dir(Some(cache_dir.to_path_buf()));
    storage.set_network(network);
    storage
  }

  async fn seed_wal_database(path: &Path) {
    ensure_parent(path).unwrap();
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

  async fn note_body(database: &Path) -> String {
    let options = SqliteConnectOptions::new().filename(database);
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    let body: String = sqlx::query_scalar("SELECT body FROM note")
      .fetch_one(&mut connection)
      .await
      .unwrap();
    connection.close().await.unwrap();
    body
  }

  mod transition {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_each_mode_pair_to_a_case() {
      assert_eq!(
        transition(StorageMode::Direct, StorageMode::Direct),
        Transition::DirectToDirect
      );
      assert_eq!(
        transition(StorageMode::Direct, StorageMode::Sync),
        Transition::DirectToSync
      );
      assert_eq!(
        transition(StorageMode::Sync, StorageMode::Direct),
        Transition::SyncToDirect
      );
      assert_eq!(transition(StorageMode::Sync, StorageMode::Sync), Transition::SyncToSync);
    }
  }

  mod direct_to_sync {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_seeds_the_share_and_a_working_copy_and_removes_the_old_direct_db() {
      let root = tempdir().unwrap();
      let local_db = root.path().join("local");
      let share = root.path().join("share");
      let cache = root.path().join("cache");
      let old = config(&local_db, &cache, false);
      let new = config(&share, &cache, true);
      fs::create_dir_all(&local_db).unwrap();
      fs::write(old.resolved_database_path(), b"live bytes").unwrap();

      migrate(&old, &new, StorageMode::Direct, StorageMode::Sync)
        .await
        .unwrap();

      assert_eq!(fs::read(new.resolved_database_path()).unwrap(), b"live bytes");
      assert_eq!(fs::read(new.resolved_working_copy_path()).unwrap(), b"live bytes");
      assert!(read_generation(&generation_marker(&new.resolved_database_path())) >= 1);
      assert_eq!(
        read_generation(&generation_marker(&new.resolved_working_copy_path())),
        read_generation(&generation_marker(&new.resolved_database_path())),
        "the working copy starts in step with the share so the first launch pulls nothing"
      );
      assert!(
        !old.resolved_database_path().exists(),
        "the old direct database is torn down, leaving no duplicate"
      );
    }

    #[tokio::test]
    async fn toggling_sync_on_in_place_seeds_a_working_copy_and_keeps_the_canonical() {
      let root = tempdir().unwrap();
      let db_dir = root.path().join("data");
      let cache = root.path().join("cache");
      // The configured path is unchanged — only the sync flag flips, as when toggling the checkbox.
      let old = config(&db_dir, &cache, false);
      let new = config(&db_dir, &cache, true);
      fs::create_dir_all(&db_dir).unwrap();
      fs::write(old.resolved_database_path(), b"live bytes").unwrap();

      migrate(&old, &new, StorageMode::Direct, StorageMode::Sync)
        .await
        .unwrap();

      assert_eq!(
        fs::read(new.resolved_database_path()).unwrap(),
        b"live bytes",
        "the in-place canonical copy is preserved when the path does not move"
      );
      assert_eq!(fs::read(new.resolved_working_copy_path()).unwrap(), b"live bytes");
      assert_eq!(
        read_generation(&generation_marker(&new.resolved_working_copy_path())),
        read_generation(&generation_marker(&new.resolved_database_path())),
      );
    }

    #[tokio::test]
    async fn a_fresh_install_with_no_database_is_a_no_op() {
      let root = tempdir().unwrap();
      let old = config(&root.path().join("local"), &root.path().join("cache"), false);
      let new = config(&root.path().join("share"), &root.path().join("cache"), true);

      migrate(&old, &new, StorageMode::Direct, StorageMode::Sync)
        .await
        .unwrap();

      assert!(!new.resolved_database_path().exists());
      assert!(!new.resolved_working_copy_path().exists());
    }
  }

  mod sync_to_direct {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_consolidates_the_working_copy_into_one_file_with_no_sidecars() {
      let root = tempdir().unwrap();
      let share = root.path().join("share");
      let cache = root.path().join("cache");
      let local_db = root.path().join("local");
      let old = config(&share, &cache, true);
      let new = config(&local_db, &cache, false);
      seed_wal_database(&old.resolved_working_copy_path()).await;
      fs::create_dir_all(&share).unwrap();
      fs::write(old.resolved_database_path(), b"stale share").unwrap();
      write_generation(&generation_marker(&old.resolved_database_path()), 3).unwrap();
      write_generation(&generation_marker(&old.resolved_working_copy_path()), 5).unwrap();

      migrate(&old, &new, StorageMode::Sync, StorageMode::Direct)
        .await
        .unwrap();

      assert_eq!(note_body(&new.resolved_database_path()).await, "hello");
      assert!(
        !with_suffix(&new.resolved_database_path(), "-wal").exists(),
        "the consolidated file carries no wal"
      );
      assert!(
        !with_suffix(&new.resolved_database_path(), "-shm").exists(),
        "the consolidated file carries no shm"
      );
      assert!(
        !old.resolved_working_copy_path().exists(),
        "the working copy is removed — no leftover second multi-GB file"
      );
      assert!(
        !old.resolved_database_path().exists(),
        "the old canonical share copy is removed too"
      );
      assert!(
        !generation_marker(&old.resolved_working_copy_path()).exists(),
        "the local marker is cleaned up"
      );
      assert!(
        !generation_marker(&old.resolved_database_path()).exists(),
        "the share sidecar is cleaned up"
      );
    }

    #[tokio::test]
    async fn toggling_sync_off_in_place_consolidates_into_the_canonical_path() {
      let root = tempdir().unwrap();
      let db_dir = root.path().join("data");
      let cache = root.path().join("cache");
      // Only the sync flag flips; the configured path stays put, as when un-checking the box.
      let old = config(&db_dir, &cache, true);
      let new = config(&db_dir, &cache, false);
      seed_wal_database(&old.resolved_working_copy_path()).await;
      fs::create_dir_all(&db_dir).unwrap();
      fs::write(old.resolved_database_path(), b"stale share").unwrap();
      write_generation(&generation_marker(&old.resolved_database_path()), 4).unwrap();
      write_generation(&generation_marker(&old.resolved_working_copy_path()), 6).unwrap();

      migrate(&old, &new, StorageMode::Sync, StorageMode::Direct)
        .await
        .unwrap();

      assert_eq!(
        note_body(&new.resolved_database_path()).await,
        "hello",
        "the freshest working-copy data lands in the canonical path"
      );
      assert!(
        !old.resolved_working_copy_path().exists(),
        "the working copy is torn down — no leftover second file"
      );
      assert!(
        !generation_marker(&new.resolved_database_path()).exists(),
        "the now-meaningless share sidecar is removed"
      );
      assert!(
        !with_suffix(&new.resolved_database_path(), "-wal").exists(),
        "no wal trails the consolidated file"
      );
    }

    #[tokio::test]
    async fn it_falls_back_to_the_share_copy_when_no_working_copy_exists() {
      let root = tempdir().unwrap();
      let share = root.path().join("share");
      let cache = root.path().join("cache");
      let local_db = root.path().join("local");
      let old = config(&share, &cache, true);
      let new = config(&local_db, &cache, false);
      seed_wal_database(&old.resolved_database_path()).await;

      migrate(&old, &new, StorageMode::Sync, StorageMode::Direct)
        .await
        .unwrap();

      assert_eq!(note_body(&new.resolved_database_path()).await, "hello");
      assert!(!old.resolved_database_path().exists());
    }
  }

  mod sync_to_sync {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_carries_the_canonical_sidecar_and_working_copy_to_the_new_share() {
      let root = tempdir().unwrap();
      let cache = root.path().join("cache");
      let old = config(&root.path().join("share-a"), &cache, true);
      let new = config(&root.path().join("share-b"), &cache, true);
      fs::create_dir_all(old.resolved_database_path().parent().unwrap()).unwrap();
      fs::write(old.resolved_database_path(), b"canonical").unwrap();
      write_generation(&generation_marker(&old.resolved_database_path()), 6).unwrap();
      fs::create_dir_all(old.resolved_working_copy_path().parent().unwrap()).unwrap();
      fs::write(old.resolved_working_copy_path(), b"working").unwrap();

      migrate(&old, &new, StorageMode::Sync, StorageMode::Sync).await.unwrap();

      assert_eq!(fs::read(new.resolved_database_path()).unwrap(), b"canonical");
      assert_eq!(read_generation(&generation_marker(&new.resolved_database_path())), 6);
      assert!(!old.resolved_database_path().exists(), "the old share copy is moved");
    }

    #[tokio::test]
    async fn it_moves_the_working_copy_and_its_wal_sidecars_when_the_cache_path_changes() {
      let root = tempdir().unwrap();
      let old = config(&root.path().join("share-a"), &root.path().join("cache-a"), true);
      let new = config(&root.path().join("share-b"), &root.path().join("cache-b"), true);
      fs::create_dir_all(old.resolved_database_path().parent().unwrap()).unwrap();
      fs::write(old.resolved_database_path(), b"canonical").unwrap();
      fs::create_dir_all(old.resolved_working_copy_path().parent().unwrap()).unwrap();
      fs::write(old.resolved_working_copy_path(), b"working").unwrap();
      write_generation(&generation_marker(&old.resolved_working_copy_path()), 9).unwrap();
      fs::write(with_suffix(&old.resolved_working_copy_path(), "-wal"), b"wal").unwrap();
      fs::write(with_suffix(&old.resolved_working_copy_path(), "-shm"), b"shm").unwrap();

      migrate(&old, &new, StorageMode::Sync, StorageMode::Sync).await.unwrap();

      assert_eq!(fs::read(new.resolved_working_copy_path()).unwrap(), b"working");
      assert_eq!(
        read_generation(&generation_marker(&new.resolved_working_copy_path())),
        9
      );
      assert_eq!(
        fs::read(with_suffix(&new.resolved_working_copy_path(), "-wal")).unwrap(),
        b"wal"
      );
      assert!(
        !old.resolved_working_copy_path().exists(),
        "the old working copy is moved, not left behind"
      );
      assert!(!with_suffix(&old.resolved_working_copy_path(), "-shm").exists());
    }

    #[tokio::test]
    async fn it_is_a_no_op_when_the_share_path_is_unchanged() {
      let root = tempdir().unwrap();
      let share = root.path().join("share");
      let cache = root.path().join("cache");
      let old = config(&share, &cache, true);
      let new = config(&share, &cache, true);
      fs::create_dir_all(old.resolved_database_path().parent().unwrap()).unwrap();
      fs::write(old.resolved_database_path(), b"canonical").unwrap();

      migrate(&old, &new, StorageMode::Sync, StorageMode::Sync).await.unwrap();

      assert_eq!(
        fs::read(old.resolved_database_path()).unwrap(),
        b"canonical",
        "an in-place share keeps its canonical copy untouched"
      );
    }
  }

  mod direct_to_direct {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_moves_the_database_in_place_without_a_working_copy() {
      let root = tempdir().unwrap();
      let cache = root.path().join("cache");
      let old = config(&root.path().join("a"), &cache, false);
      let new = config(&root.path().join("b"), &cache, false);
      fs::create_dir_all(old.resolved_database_path().parent().unwrap()).unwrap();
      fs::write(old.resolved_database_path(), b"db").unwrap();

      migrate(&old, &new, StorageMode::Direct, StorageMode::Direct)
        .await
        .unwrap();

      assert_eq!(fs::read(new.resolved_database_path()).unwrap(), b"db");
      assert!(!old.resolved_database_path().exists());
      assert!(
        !new.resolved_working_copy_path().exists(),
        "direct mode keeps no working copy"
      );
    }
  }

  mod abort_safety {
    use super::*;

    struct Guard {
      _dir: TempDir,
    }

    #[tokio::test]
    async fn a_failed_sync_to_direct_leaves_the_old_layout_intact() {
      let root = tempdir().unwrap();
      let _guard = Guard {
        _dir: root,
      };
      let dir = _guard._dir.path();
      let share = dir.join("share");
      let cache = dir.join("cache");
      let old = config(&share, &cache, true);
      // Point the new local location at a path whose parent cannot be created (a file blocks it),
      // so checkpoint_into fails and the migration must abort without destroying the old layout.
      let blocker = dir.join("blocked");
      fs::write(&blocker, b"i am a file, not a directory").unwrap();
      let mut new = StorageConfig::default();
      new.set_db_dir(Some(blocker.join("nested")));
      new.set_cache_dir(Some(cache.clone()));
      seed_wal_database(&old.resolved_working_copy_path()).await;
      write_generation(&generation_marker(&old.resolved_working_copy_path()), 5).unwrap();

      let result = migrate(&old, &new, StorageMode::Sync, StorageMode::Direct).await;

      assert!(result.is_err(), "the migration reports the failure");
      assert!(
        old.resolved_working_copy_path().exists(),
        "the working copy — the freshest data — survives a failed switch"
      );
      assert!(
        generation_marker(&old.resolved_working_copy_path()).exists(),
        "its marker survives too"
      );
    }

    #[tokio::test]
    async fn a_failed_direct_to_sync_keeps_the_old_direct_database() {
      let root = tempdir().unwrap();
      let _guard = Guard {
        _dir: root,
      };
      let dir = _guard._dir.path();
      let local_db = dir.join("local");
      let cache = dir.join("cache");
      let old = config(&local_db, &cache, false);
      fs::create_dir_all(&local_db).unwrap();
      fs::write(old.resolved_database_path(), b"live bytes").unwrap();
      // A file where the share directory must go forces the copy onto the share to fail.
      let blocker = dir.join("share-blocker");
      fs::write(&blocker, b"blocking file").unwrap();
      let mut new = StorageConfig::default();
      new.set_db_dir(Some(blocker.join("nested")));
      new.set_cache_dir(Some(cache));
      new.set_network(true);

      let result = migrate(&old, &new, StorageMode::Direct, StorageMode::Sync).await;

      assert!(result.is_err());
      assert_eq!(
        fs::read(old.resolved_database_path()).unwrap(),
        b"live bytes",
        "the original direct database is untouched after a failed switch"
      );
    }
  }
}
