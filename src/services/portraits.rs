//! Portrait byte cache: read from and write to the local filesystem.

use std::path::PathBuf;

pub fn load(char_id: i64) -> Option<Vec<u8>> {
  std::fs::read(path(char_id)).ok()
}

pub fn save(char_id: i64, bytes: &[u8]) {
  let p = path(char_id);
  if let Some(dir) = p.parent() {
    let _ = std::fs::create_dir_all(dir);
  }
  let _ = std::fs::write(p, bytes);
}

fn path(char_id: i64) -> PathBuf {
  dir_spec::cache_home()
    .map(|p| p.join("pod").join("portraits").join(format!("{char_id}.png")))
    .expect("failed to resolve cache directory")
}
