//! ESI disk-cache management.

use std::path::PathBuf;

pub async fn clear_esi_cache() -> Result<usize, std::io::Error> {
  let Some(cache_dir) = cache_path() else {
    return Ok(0);
  };

  let mut read_dir = match tokio::fs::read_dir(&cache_dir).await {
    Ok(rd) => rd,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
    Err(e) => return Err(e),
  };

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
