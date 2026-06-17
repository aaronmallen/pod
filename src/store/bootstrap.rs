use std::{
  fs, io,
  path::{Path, PathBuf},
};

use crate::{
  config::{StorageConfig, StorageMode},
  store::{reconcile, sync_copy::SyncCopy},
};

/// errno 18: rename refused because source and destination live on different filesystems, which
/// triggers the copy-then-delete fallback in `move_file`.
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

pub fn resolve_local_path(storage: &StorageConfig) -> Result<PathBuf, Error> {
  let canonical = storage.resolved_database_path();
  let working_copy = storage.resolved_working_copy_path();
  match storage.storage_mode() {
    StorageMode::Direct => {
      let resolved = resolve_direct(&canonical, &working_copy)?;
      // After resolve so a lingering working copy is adopted before stale Sync artifacts are removed.
      reconcile::clean_direct_artifacts(&canonical, &working_copy);
      Ok(resolved)
    }
    StorageMode::Sync => {
      reconcile::reconcile_sync(&canonical, &working_copy)?;
      resolve_sync(&canonical, &working_copy)
    }
  }
}

fn resolve_direct(canonical: &Path, working_copy: &Path) -> Result<PathBuf, Error> {
  ensure_parent(canonical)?;

  if !canonical.exists() && working_copy.exists() {
    // Move the -wal/-shm sidecars first: relocating the bare .db would let SQLite discard the
    // orphaned WAL at the old path and silently lose any uncheckpointed writes.
    for suffix in WAL_SIDECARS {
      move_sidecar(working_copy, canonical, suffix)?;
    }
    move_file(working_copy, canonical)?;
    remove_generation_marker(working_copy);
  }

  Ok(canonical.to_path_buf())
}

fn resolve_sync(canonical: &Path, working_copy: &Path) -> Result<PathBuf, Error> {
  ensure_parent(working_copy)?;

  let engine = SyncCopy::new(
    canonical.to_path_buf(),
    generation_marker(canonical),
    working_copy.to_path_buf(),
    generation_marker(working_copy),
  );
  engine.pull_if_newer()?;

  Ok(working_copy.to_path_buf())
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

fn remove_generation_marker(database: &Path) {
  let _ = fs::remove_file(generation_marker(database));
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
  let mut name = path.as_os_str().to_owned();
  name.push(suffix);
  PathBuf::from(name)
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;

  use super::*;
  use crate::store::share_meta::{read_generation, write_generation};

  mod generation_marker {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_appends_the_generation_suffix_beside_the_database() {
      let marker = generation_marker(Path::new("/data/pod.db"));

      assert_eq!(marker, PathBuf::from("/data/pod.db.generation"));
    }
  }

  mod resolve_direct {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_carries_the_wal_sidecars_alongside_a_relocated_working_copy() {
      let dir = tempdir().unwrap();
      let canonical = dir.path().join("data").join("pod.db");
      let working_copy = dir.path().join("cache").join("db").join("pod.db");
      fs::create_dir_all(working_copy.parent().unwrap()).unwrap();
      fs::write(&working_copy, b"db").unwrap();
      fs::write(with_suffix(&working_copy, "-wal"), b"wal").unwrap();
      fs::write(with_suffix(&working_copy, "-shm"), b"shm").unwrap();

      resolve_direct(&canonical, &working_copy).unwrap();

      assert_eq!(fs::read(with_suffix(&canonical, "-wal")).unwrap(), b"wal");
      assert_eq!(fs::read(with_suffix(&canonical, "-shm")).unwrap(), b"shm");
      assert!(
        !with_suffix(&working_copy, "-wal").exists(),
        "the wal is moved, not left behind"
      );
    }

    #[test]
    fn it_creates_the_canonical_parent_for_a_fresh_install() {
      let dir = tempdir().unwrap();
      let canonical = dir.path().join("data").join("pod.db");
      let working_copy = dir.path().join("cache").join("db").join("pod.db");

      let resolved = resolve_direct(&canonical, &working_copy).unwrap();

      assert_eq!(resolved, canonical);
      assert!(canonical.parent().unwrap().is_dir());
    }

    #[test]
    fn it_leaves_a_stale_working_copy_untouched_when_the_canonical_already_exists() {
      let dir = tempdir().unwrap();
      let canonical = dir.path().join("data").join("pod.db");
      let working_copy = dir.path().join("cache").join("db").join("pod.db");
      fs::create_dir_all(canonical.parent().unwrap()).unwrap();
      fs::create_dir_all(working_copy.parent().unwrap()).unwrap();
      fs::write(&canonical, b"live").unwrap();
      fs::write(&working_copy, b"stale").unwrap();

      resolve_direct(&canonical, &working_copy).unwrap();

      assert_eq!(fs::read(&canonical).unwrap(), b"live", "the live file wins");
    }

    #[test]
    fn it_moves_a_lingering_working_copy_into_place_when_the_canonical_is_absent() {
      let dir = tempdir().unwrap();
      let canonical = dir.path().join("data").join("pod.db");
      let working_copy = dir.path().join("cache").join("db").join("pod.db");
      fs::create_dir_all(working_copy.parent().unwrap()).unwrap();
      fs::write(&working_copy, b"recovered").unwrap();
      write_generation(&generation_marker(&working_copy), 4).unwrap();

      let resolved = resolve_direct(&canonical, &working_copy).unwrap();

      assert_eq!(resolved, canonical);
      assert_eq!(fs::read(&canonical).unwrap(), b"recovered");
      assert!(!working_copy.exists(), "the working copy is moved, not duplicated");
      assert!(!generation_marker(&working_copy).exists(), "its marker is cleaned up");
    }

    #[test]
    fn it_opens_the_canonical_path_in_place_and_creates_no_working_copy() {
      let dir = tempdir().unwrap();
      let canonical = dir.path().join("data").join("pod.db");
      let working_copy = dir.path().join("cache").join("db").join("pod.db");
      fs::create_dir_all(canonical.parent().unwrap()).unwrap();
      fs::write(&canonical, b"live").unwrap();

      let resolved = resolve_direct(&canonical, &working_copy).unwrap();

      assert_eq!(resolved, canonical);
      assert!(!working_copy.exists(), "direct mode never creates a working copy");
      assert_eq!(fs::read(&canonical).unwrap(), b"live");
    }
  }

  mod resolve_sync {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_opens_the_working_copy_and_pulls_when_the_share_is_newer() {
      let dir = tempdir().unwrap();
      let canonical = dir.path().join("share").join("pod.db");
      let working_copy = dir.path().join("cache").join("db").join("pod.db");
      fs::create_dir_all(canonical.parent().unwrap()).unwrap();
      fs::write(&canonical, b"share bytes").unwrap();
      write_generation(&generation_marker(&canonical), 5).unwrap();

      let resolved = resolve_sync(&canonical, &working_copy).unwrap();

      assert_eq!(resolved, working_copy);
      assert_eq!(fs::read(&working_copy).unwrap(), b"share bytes");
      assert_eq!(read_generation(&generation_marker(&working_copy)), 5);
    }

    #[test]
    fn it_pulls_nothing_when_the_generations_match() {
      let dir = tempdir().unwrap();
      let canonical = dir.path().join("share").join("pod.db");
      let working_copy = dir.path().join("cache").join("db").join("pod.db");
      fs::create_dir_all(canonical.parent().unwrap()).unwrap();
      fs::create_dir_all(working_copy.parent().unwrap()).unwrap();
      fs::write(&canonical, b"share bytes").unwrap();
      fs::write(&working_copy, b"local bytes").unwrap();
      write_generation(&generation_marker(&canonical), 3).unwrap();
      write_generation(&generation_marker(&working_copy), 3).unwrap();

      let resolved = resolve_sync(&canonical, &working_copy).unwrap();

      assert_eq!(resolved, working_copy);
      assert_eq!(
        fs::read(&working_copy).unwrap(),
        b"local bytes",
        "a same-machine relaunch performs no download"
      );
    }
  }
}
