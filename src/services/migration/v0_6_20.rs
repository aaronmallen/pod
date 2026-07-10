use std::path::{Path, PathBuf};

use cargo_packager_updater::semver::Version;
use toml_edit::DocumentMut;

use super::{Error, Migrator, Result};
use crate::{
  config::{self, StorageConfig, StorageMode},
  features::settings::storage_tab,
  store::storage_migration,
};

const EXDEV: i32 = 18;
const STATE_SIBLINGS: [&str; 3] = ["sde_version", "synced_language", "window.json"];
const WORKING_COPY_SUBDIR: &str = "db";

fn move_error(error: impl std::fmt::Display) -> Error {
  Error::Config(error.to_string())
}

#[allow(non_camel_case_types)]
pub(super) struct V0_6_20;

impl Migrator for V0_6_20 {
  fn version(&self) -> Version {
    Version::new(0, 6, 20)
  }

  async fn before_startup(&self) -> Result<()> {
    move_config()?;
    let settings = config::load().map_err(move_error)?;
    let storage = settings.storage();
    relocate_caches(storage)?;
    relocate_db_and_state(storage).await
  }
}

fn relocate_caches(storage: &StorageConfig) -> Result<()> {
  move_cache(storage)?;
  move_logs(storage)
}

async fn relocate_db_and_state(storage: &StorageConfig) -> Result<()> {
  move_database(storage).await?;
  move_siblings()?;
  cleanup_legacy_state();
  Ok(())
}

fn move_config() -> Result<()> {
  let Ok(new_path) = config::config_path() else {
    return Ok(());
  };
  move_config_files(&new_path, config::legacy_config_path().as_deref())
}

fn move_config_files(new_path: &Path, old_path: Option<&Path>) -> Result<()> {
  let Some(old_path) = old_path else {
    return Ok(());
  };
  if paths_equal(new_path, old_path) {
    return Ok(());
  }
  if new_path.exists() {
    let _ = std::fs::remove_file(old_path);
    return Ok(());
  }
  if !old_path.exists() {
    return Ok(());
  }
  if let Some(parent) = new_path.parent() {
    std::fs::create_dir_all(parent).map_err(move_error)?;
  }
  let content = std::fs::read_to_string(old_path).map_err(move_error)?;
  let rendered = content
    .parse::<DocumentMut>()
    .map(|document| document.to_string())
    .unwrap_or(content);
  std::fs::write(new_path, rendered).map_err(move_error)?;
  std::fs::remove_file(old_path).map_err(move_error)
}

fn move_cache(storage: &StorageConfig) -> Result<()> {
  relocate_knob(storage.cache_dir(), &config::legacy_cache_dir(), &config::cache_dir())
}

fn move_logs(storage: &StorageConfig) -> Result<()> {
  relocate_knob(storage.log_dir(), &config::legacy_log_dir(), &config::log_dir())
}

fn relocate_knob(knob: &Option<PathBuf>, old: &Path, new: &Path) -> Result<()> {
  if knob.is_some() {
    return Ok(());
  }
  move_dir(old, new)
}

async fn move_database(storage: &StorageConfig) -> Result<()> {
  if storage.db_dir().is_some() {
    return Ok(());
  }
  let mode = storage.storage_mode();
  relocate_database(&legacy_db_config(storage), &new_db_config(storage), mode).await
}

async fn relocate_database(old: &StorageConfig, new: &StorageConfig, mode: StorageMode) -> Result<()> {
  storage_migration::migrate(old, new, mode, mode)
    .await
    .map_err(move_error)
}

fn legacy_db_config(storage: &StorageConfig) -> StorageConfig {
  let mut cfg = StorageConfig::default();
  cfg.set_db_dir(Some(config::legacy_data_dir()));
  cfg.set_network(*storage.network());
  cfg
}

fn new_db_config(storage: &StorageConfig) -> StorageConfig {
  let mut cfg = StorageConfig::default();
  cfg.set_network(*storage.network());
  cfg
}

fn move_siblings() -> Result<()> {
  let (Some(old_state), Some(new_state)) = (config::legacy_state_dir(), config::state_dir()) else {
    return Ok(());
  };
  move_state_tree(&old_state, &new_state)
}

fn move_state_tree(old_state: &Path, new_state: &Path) -> Result<()> {
  if paths_equal(old_state, new_state) {
    return Ok(());
  }
  for name in STATE_SIBLINGS {
    move_file(&old_state.join(name), &new_state.join(name))?;
  }
  move_dir(
    &old_state.join(WORKING_COPY_SUBDIR),
    &new_state.join(WORKING_COPY_SUBDIR),
  )
}

fn cleanup_legacy_state() {
  if let Some(old_state) = config::legacy_state_dir() {
    let _ = std::fs::remove_dir(old_state);
  }
}

fn move_dir(old: &Path, new: &Path) -> Result<()> {
  if !old.exists() || paths_equal(old, new) {
    return Ok(());
  }
  storage_tab::relocate(old, new).map_err(move_error)
}

fn move_file(old: &Path, new: &Path) -> Result<()> {
  if !old.exists() || paths_equal(old, new) {
    return Ok(());
  }
  if new.exists() {
    return std::fs::remove_file(old).map_err(move_error);
  }
  if let Some(parent) = new.parent() {
    std::fs::create_dir_all(parent).map_err(move_error)?;
  }
  match std::fs::rename(old, new) {
    Ok(()) => Ok(()),
    Err(error) if error.raw_os_error() == Some(EXDEV) => {
      std::fs::copy(old, new).map_err(move_error)?;
      std::fs::remove_file(old).map_err(move_error)
    }
    Err(error) => Err(move_error(error)),
  }
}

fn paths_equal(a: &Path, b: &Path) -> bool {
  match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
    (Ok(a), Ok(b)) => a == b,
    _ => a == b,
  }
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;

  use super::*;

  fn write(path: &Path, body: &[u8]) {
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
  }

  mod relocate_knob {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_moves_a_default_directory_when_the_knob_is_none() {
      let root = tempdir().unwrap();
      let old = root.path().join("old");
      let new = root.path().join("new");
      write(&old.join("image.png"), b"bytes");

      relocate_knob(&None, &old, &new).unwrap();

      assert_eq!(std::fs::read(new.join("image.png")).unwrap(), b"bytes");
      assert!(!old.exists(), "the old directory is removed after the move");
    }

    #[test]
    fn it_leaves_a_customized_directory_untouched_when_the_knob_is_some() {
      let root = tempdir().unwrap();
      let old = root.path().join("old");
      let new = root.path().join("new");
      write(&old.join("image.png"), b"bytes");

      relocate_knob(&Some(old.clone()), &old, &new).unwrap();

      assert!(old.join("image.png").exists(), "a customized path is never relocated");
      assert!(!new.exists());
    }
  }

  mod move_dir {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_a_no_op_when_the_source_is_absent() {
      let root = tempdir().unwrap();
      move_dir(&root.path().join("missing"), &root.path().join("new")).unwrap();
      assert!(!root.path().join("new").exists());
    }

    #[test]
    fn a_second_run_after_a_completed_move_is_a_no_op() {
      let root = tempdir().unwrap();
      let old = root.path().join("old");
      let new = root.path().join("new");
      write(&old.join("a.log"), b"one");

      move_dir(&old, &new).unwrap();
      move_dir(&old, &new).unwrap();

      assert_eq!(std::fs::read(new.join("a.log")).unwrap(), b"one");
      assert!(!old.exists());
    }

    #[test]
    fn a_failed_move_leaves_the_source_intact() {
      let root = tempdir().unwrap();
      let old = root.path().join("old");
      write(&old.join("a.log"), b"one");
      let blocker = root.path().join("blocker");
      write(&blocker, b"i am a file");
      let new = blocker.join("nested").join("new");

      assert!(move_dir(&old, &new).is_err(), "creating a dir under a file fails");
      assert_eq!(
        std::fs::read(old.join("a.log")).unwrap(),
        b"one",
        "the source survives a failed relocation"
      );
    }
  }

  mod move_file {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_moves_a_file_and_removes_the_source() {
      let root = tempdir().unwrap();
      let old = root.path().join("old/sde_version");
      let new = root.path().join("new/sde_version");
      write(&old, b"marker");

      move_file(&old, &new).unwrap();

      assert_eq!(std::fs::read(&new).unwrap(), b"marker");
      assert!(!old.exists());
    }

    #[test]
    fn it_prefers_the_new_file_and_drops_the_stale_source_on_re_run() {
      let root = tempdir().unwrap();
      let old = root.path().join("old/window.json");
      let new = root.path().join("new/window.json");
      write(&old, b"stale");
      write(&new, b"current");

      move_file(&old, &new).unwrap();

      assert_eq!(std::fs::read(&new).unwrap(), b"current", "the already-moved file wins");
      assert!(!old.exists(), "the stale source is cleaned up");
    }

    #[test]
    fn it_is_a_no_op_when_the_source_is_absent() {
      let root = tempdir().unwrap();
      move_file(&root.path().join("missing"), &root.path().join("new")).unwrap();
      assert!(!root.path().join("new").exists());
    }
  }

  mod move_config_files {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_moves_the_config_preserving_comments_when_the_new_path_is_empty() {
      let root = tempdir().unwrap();
      let old = root.path().join("old/config.toml");
      let new = root.path().join("new/config.toml");
      write(&old, b"# keep me\nnetwork = true\n");

      move_config_files(&new, Some(&old)).unwrap();

      let moved = std::fs::read_to_string(&new).unwrap();
      assert!(moved.contains("# keep me"), "comments survive: {moved}");
      assert!(moved.contains("network = true"), "values survive: {moved}");
      assert!(!old.exists(), "the old config is removed");
    }

    #[test]
    fn it_prefers_the_new_config_and_drops_the_stale_old_one() {
      let root = tempdir().unwrap();
      let old = root.path().join("old/config.toml");
      let new = root.path().join("new/config.toml");
      write(&old, b"network = false\n");
      write(&new, b"network = true\n");

      move_config_files(&new, Some(&old)).unwrap();

      assert_eq!(
        std::fs::read_to_string(&new).unwrap(),
        "network = true\n",
        "the migrated config is authoritative"
      );
      assert!(!old.exists(), "the stale legacy config is removed");
    }

    #[test]
    fn it_is_a_no_op_when_no_config_exists_anywhere() {
      let root = tempdir().unwrap();
      let new = root.path().join("new/config.toml");
      move_config_files(&new, Some(&root.path().join("old/config.toml"))).unwrap();
      assert!(!new.exists());
    }
  }

  mod move_state_tree {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_carries_the_siblings_and_working_copy_to_the_new_state_home() {
      let root = tempdir().unwrap();
      let old = root.path().join("state/pod");
      let new = root.path().join("state/dev.aaronmallen.pod");
      write(&old.join("sde_version"), b"1234+pod-0.6.19");
      write(&old.join("synced_language"), b"en");
      write(&old.join("window.json"), b"{}");
      write(&old.join("db/pod.db"), b"working copy");

      move_state_tree(&old, &new).unwrap();

      assert_eq!(std::fs::read(new.join("sde_version")).unwrap(), b"1234+pod-0.6.19");
      assert_eq!(std::fs::read(new.join("synced_language")).unwrap(), b"en");
      assert_eq!(std::fs::read(new.join("window.json")).unwrap(), b"{}");
      assert_eq!(std::fs::read(new.join("db/pod.db")).unwrap(), b"working copy");
      assert!(!old.join("db").exists(), "the working copy is moved, not split");
    }

    #[test]
    fn a_second_run_after_a_completed_move_is_a_no_op() {
      let root = tempdir().unwrap();
      let old = root.path().join("state/pod");
      let new = root.path().join("state/dev.aaronmallen.pod");
      write(&old.join("sde_version"), b"marker");
      write(&old.join("db/pod.db"), b"wc");

      move_state_tree(&old, &new).unwrap();
      move_state_tree(&old, &new).unwrap();

      assert_eq!(std::fs::read(new.join("sde_version")).unwrap(), b"marker");
      assert_eq!(std::fs::read(new.join("db/pod.db")).unwrap(), b"wc");
    }
  }

  mod move_database {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn a_customized_db_dir_is_never_relocated() {
      let root = tempdir().unwrap();
      let mut storage = StorageConfig::default();
      storage.set_db_dir(Some(root.path().join("custom")));
      write(&storage.resolved_database_path(), b"custom bytes");

      move_database(&storage).await.unwrap();

      assert_eq!(
        std::fs::read(storage.resolved_database_path()).unwrap(),
        b"custom bytes",
        "a user-set db_dir is left byte-identical"
      );
    }

    #[tokio::test]
    async fn it_relocates_a_default_direct_database_between_locations() {
      let root = tempdir().unwrap();
      let mut old = StorageConfig::default();
      old.set_db_dir(Some(root.path().join("old")));
      let mut new = StorageConfig::default();
      new.set_db_dir(Some(root.path().join("new")));
      write(&old.resolved_database_path(), b"live bytes");

      relocate_database(&old, &new, StorageMode::Direct).await.unwrap();

      assert_eq!(std::fs::read(new.resolved_database_path()).unwrap(), b"live bytes");
      assert!(!old.resolved_database_path().exists(), "the old database is torn down");
    }

    #[tokio::test]
    async fn a_failed_relocation_leaves_the_old_database_intact() {
      let root = tempdir().unwrap();
      let mut old = StorageConfig::default();
      old.set_db_dir(Some(root.path().join("old")));
      write(&old.resolved_database_path(), b"live bytes");
      let blocker = root.path().join("blocker");
      write(&blocker, b"i am a file");
      let mut new = StorageConfig::default();
      new.set_db_dir(Some(blocker.join("nested")));

      assert!(relocate_database(&old, &new, StorageMode::Direct).await.is_err());
      assert_eq!(
        std::fs::read(old.resolved_database_path()).unwrap(),
        b"live bytes",
        "the original database survives a failed relocation"
      );
    }
  }
}
