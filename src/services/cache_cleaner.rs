use std::{fs, io, path::Path};

use crate::config::StorageConfig;

pub fn clear(storage: &StorageConfig) -> io::Result<()> {
  clear_dir(&storage.resolved_cache_dir())
}

fn clear_dir(dir: &Path) -> io::Result<()> {
  let entries = match fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
    Err(error) => return Err(error),
  };

  for entry in entries {
    let entry = entry?;
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      fs::remove_dir_all(&path)?;
    } else {
      fs::remove_file(&path)?;
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use super::*;

  fn storage_with_cache(dir: PathBuf) -> StorageConfig {
    let mut storage = StorageConfig::default();
    storage.set_cache_dir(Some(dir));
    storage
  }

  #[test]
  fn it_removes_files_and_subdirs_from_the_cache() {
    let temp = tempfile::tempdir().unwrap();
    let cache = temp.path();
    fs::write(cache.join("portrait.png"), b"image").unwrap();
    fs::create_dir(cache.join("esi")).unwrap();
    fs::write(cache.join("esi").join("response.json"), b"{}").unwrap();

    clear(&storage_with_cache(cache.to_path_buf())).unwrap();

    assert!(cache.exists(), "the cache directory itself is preserved");
    assert_eq!(fs::read_dir(cache).unwrap().count(), 0, "the cache is emptied");
  }

  #[test]
  fn it_is_ok_when_the_cache_is_already_empty() {
    let temp = tempfile::tempdir().unwrap();

    clear(&storage_with_cache(temp.path().to_path_buf())).unwrap();

    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
  }

  #[test]
  fn it_is_ok_when_the_cache_directory_is_absent() {
    let temp = tempfile::tempdir().unwrap();
    let cache = temp.path().join("does-not-exist");

    clear(&storage_with_cache(cache.clone())).unwrap();

    assert!(!cache.exists(), "an absent cache stays absent");
  }
}
