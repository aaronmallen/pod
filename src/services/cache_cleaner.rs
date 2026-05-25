//! ESI disk-cache management.

use std::path::PathBuf;

pub async fn clear_esi_cache() -> Result<usize, std::io::Error> {
  let Some(cache_dir) = cache_path() else {
    return Ok(0);
  };
  let read_dir = open_cache_dir(&cache_dir).await?;
  match read_dir {
    Some(rd) => remove_cache_files(rd).await,
    None => Ok(0),
  }
}

async fn open_cache_dir(cache_dir: &std::path::Path) -> Result<Option<tokio::fs::ReadDir>, std::io::Error> {
  match tokio::fs::read_dir(cache_dir).await {
    Ok(rd) => Ok(Some(rd)),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(e) => Err(e),
  }
}

async fn remove_cache_files(mut read_dir: tokio::fs::ReadDir) -> Result<usize, std::io::Error> {
  let mut count = 0usize;
  while let Some(entry) = read_dir.next_entry().await? {
    let meta = entry.metadata().await?;
    if meta.is_file() {
      tokio::fs::remove_file(entry.path()).await?;
      count += 1;
    }
  }
  Ok(count)
}

fn cache_path() -> Option<PathBuf> {
  dir_spec::cache_home().map(|p| p.join("pod"))
}
