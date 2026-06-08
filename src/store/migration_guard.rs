use std::path::{Path, PathBuf};

const SCHEMA_GENERATION_BASELINE: (u32, u32) = (0, 5);

#[derive(Clone, Debug)]
pub struct MigrationGuard {
  cache_dir: PathBuf,
  database_path: PathBuf,
  marker_path: Option<PathBuf>,
  window_path: Option<PathBuf>,
  config_path: Option<PathBuf>,
}

impl MigrationGuard {
  pub fn new(
    cache_dir: PathBuf,
    database_path: PathBuf,
    marker_path: Option<PathBuf>,
    window_path: Option<PathBuf>,
    config_path: Option<PathBuf>,
  ) -> Self {
    Self {
      cache_dir,
      database_path,
      marker_path,
      window_path,
      config_path,
    }
  }

  pub fn run(&self) {
    remove_dir(&legacy_images_dir(&crate::config::data_dir()));

    let marker = self.marker_path.as_deref().and_then(read_marker);
    if decision(marker.as_deref(), self.database_path.exists()) == Decision::Keep {
      return;
    }

    tracing::info!(target: "pod::lifecycle", "legacy install detected; backing up pre-0.5.0 state before migration");
    self.back_up();
  }

  fn back_up(&self) {
    back_up_file(&self.database_path, &backup(&self.database_path));
    remove_file(&sidecar(&self.database_path, "-wal"));
    remove_file(&sidecar(&self.database_path, "-shm"));
    remove_dir(&self.cache_dir);
    if let Some(window_path) = &self.window_path {
      remove_file(window_path);
    }
    if let Some(config_path) = &self.config_path {
      remove_file(config_path);
    }
    if let Some(marker_path) = &self.marker_path {
      remove_file(marker_path);
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decision {
  BackUp,
  Keep,
}

fn back_up_file(source: &Path, destination: &Path) {
  if !source.exists() {
    return;
  }
  if destination.exists() {
    remove_file(destination);
  }
  match std::fs::rename(source, destination) {
    Ok(()) => {
      tracing::info!(target: "pod::lifecycle", source = %source.display(), destination = %destination.display(), "backed up legacy database")
    }
    Err(error) => {
      tracing::warn!(target: "pod::lifecycle", %error, source = %source.display(), destination = %destination.display(), "failed to back up legacy database")
    }
  }
}

fn backup(path: &Path) -> PathBuf {
  let mut name = path.as_os_str().to_owned();
  name.push(".backup");
  PathBuf::from(name)
}

fn decision(marker: Option<&str>, db_exists: bool) -> Decision {
  match marker {
    Some(marker) => match parse_pod_version(marker) {
      Some(version) if version >= SCHEMA_GENERATION_BASELINE => Decision::Keep,
      _ => Decision::BackUp,
    },
    None if db_exists => Decision::BackUp,
    None => Decision::Keep,
  }
}

fn legacy_images_dir(data_dir: &Path) -> PathBuf {
  data_dir.join("images")
}

fn parse_pod_version(marker: &str) -> Option<(u32, u32)> {
  let version = marker.split("+pod-").nth(1)?.split('+').next()?;
  let mut parts = version.split('.');
  let major = parts.next()?.parse().ok()?;
  let minor = parts.next()?.parse().ok()?;
  Some((major, minor))
}

fn read_marker(path: &Path) -> Option<String> {
  let contents = std::fs::read_to_string(path).ok()?;
  Some(contents.trim().to_owned())
}

fn remove_dir(path: &Path) {
  if !path.exists() {
    return;
  }
  match std::fs::remove_dir_all(path) {
    Ok(()) => tracing::info!(target: "pod::lifecycle", path = %path.display(), "wiped legacy directory"),
    Err(error) => {
      tracing::warn!(target: "pod::lifecycle", %error, path = %path.display(), "failed to remove legacy directory")
    }
  }
}

fn remove_file(path: &Path) {
  if !path.exists() {
    return;
  }
  match std::fs::remove_file(path) {
    Ok(()) => tracing::info!(target: "pod::lifecycle", path = %path.display(), "wiped legacy file"),
    Err(error) => {
      tracing::warn!(target: "pod::lifecycle", %error, path = %path.display(), "failed to remove legacy file")
    }
  }
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
  let mut name = path.as_os_str().to_owned();
  name.push(suffix);
  PathBuf::from(name)
}

#[cfg(test)]
mod tests {
  use super::*;

  mod decision {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_wipes_for_a_0_4_9_marker() {
      let marker = "20240101.1+pod-0.4.9+seed-2";

      assert_eq!(decision(Some(marker), true), Decision::BackUp);
    }

    #[test]
    fn it_keeps_for_a_0_5_0_marker() {
      let marker = "20240101.1+pod-0.5.0+seed-2";

      assert_eq!(decision(Some(marker), true), Decision::Keep);
    }

    #[test]
    fn it_wipes_when_no_marker_but_a_db_exists() {
      assert_eq!(decision(None, true), Decision::BackUp);
    }

    #[test]
    fn it_keeps_a_fresh_install_with_no_marker_and_no_db() {
      assert_eq!(decision(None, false), Decision::Keep);
    }

    #[test]
    fn it_wipes_for_a_malformed_marker() {
      assert_eq!(decision(Some("garbage"), true), Decision::BackUp);
    }
  }

  mod parse_pod_version {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_the_pod_segment_of_a_composite_marker() {
      assert_eq!(parse_pod_version("20240101.1+pod-0.5.0+seed-2"), Some((0, 5)));
    }

    #[test]
    fn it_parses_a_two_digit_minor() {
      assert_eq!(parse_pod_version("20240101.1+pod-1.12.3+seed-2"), Some((1, 12)));
    }

    #[test]
    fn it_returns_none_when_the_pod_segment_is_missing() {
      assert_eq!(parse_pod_version("20240101.1+seed-2"), None);
    }

    #[test]
    fn it_returns_none_for_a_non_numeric_version() {
      assert_eq!(parse_pod_version("20240101.1+pod-abc+seed-2"), None);
    }

    #[test]
    fn it_returns_none_when_the_minor_is_absent() {
      assert_eq!(parse_pod_version("20240101.1+pod-1+seed-2"), None);
    }
  }

  mod legacy_images {
    use std::fs;

    use super::*;

    #[test]
    fn it_removes_the_legacy_images_directory_when_present() {
      let tmp = tempfile::tempdir().unwrap();
      let images = legacy_images_dir(tmp.path());
      fs::create_dir_all(images.join("587")).unwrap();
      fs::write(images.join("587").join("64.png"), b"x").unwrap();

      remove_dir(&images);

      assert!(!images.exists());
    }

    #[test]
    fn it_is_a_no_op_when_the_legacy_images_directory_is_absent() {
      let tmp = tempfile::tempdir().unwrap();
      let images = legacy_images_dir(tmp.path());

      remove_dir(&images);

      assert!(!images.exists());
      assert!(tmp.path().exists());
    }
  }

  mod back_up {
    use std::fs;

    use pretty_assertions::assert_eq;

    use super::*;

    fn write(path: &Path, contents: &[u8]) {
      fs::write(path, contents).unwrap();
    }

    #[test]
    fn it_backs_up_the_database_removes_other_targets_and_is_idempotent() {
      let tmp = tempfile::tempdir().unwrap();
      let base = tmp.path();
      let database_path = base.join("pod.db");
      let backup_path = backup(&database_path);
      let cache_dir = base.join("cache");
      let marker_path = base.join("sde_version");
      let window_path = base.join("window.json");
      let config_path = base.join("config.toml");

      write(&database_path, b"original");
      write(&sidecar(&database_path, "-wal"), b"x");
      write(&sidecar(&database_path, "-shm"), b"x");
      fs::create_dir(&cache_dir).unwrap();
      write(&cache_dir.join("icon.png"), b"x");
      write(&marker_path, b"x");
      write(&window_path, b"x");
      write(&config_path, b"x");

      let guard = MigrationGuard::new(
        cache_dir.clone(),
        database_path.clone(),
        Some(marker_path.clone()),
        Some(window_path.clone()),
        Some(config_path.clone()),
      );
      guard.back_up();

      assert!(!database_path.exists());
      assert!(backup_path.exists());
      assert_eq!(fs::read(&backup_path).unwrap(), b"original");
      assert!(!sidecar(&database_path, "-wal").exists());
      assert!(!sidecar(&database_path, "-shm").exists());
      assert!(!cache_dir.exists());
      assert!(!marker_path.exists());
      assert!(!window_path.exists());
      assert!(!config_path.exists());

      guard.back_up();
    }

    #[test]
    fn it_overwrites_an_existing_backup_so_one_remains() {
      let tmp = tempfile::tempdir().unwrap();
      let base = tmp.path();
      let database_path = base.join("pod.db");
      let backup_path = backup(&database_path);

      write(&database_path, b"newer");
      write(&backup_path, b"stale");

      let guard = MigrationGuard::new(base.join("cache"), database_path.clone(), None, None, None);
      guard.back_up();

      assert!(!database_path.exists());
      assert_eq!(fs::read(&backup_path).unwrap(), b"newer");
    }

    #[test]
    fn it_does_not_create_a_backup_for_a_fresh_install() {
      let tmp = tempfile::tempdir().unwrap();
      let base = tmp.path();
      let database_path = base.join("pod.db");

      let guard = MigrationGuard::new(base.join("cache"), database_path.clone(), None, None, None);
      guard.back_up();

      assert!(!backup(&database_path).exists());
    }
  }
}
