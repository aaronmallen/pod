//! Boot-time reconciliation for Sync-mode shared-drive storage.
//!
//! Called by `bootstrap::resolve_local_path` before the database is opened.
//! Ensures the working copy is never empty when a canonical exists, and resolves
//! divergence (independent writes on two machines) by generation number so the
//! losing side is backed up rather than silently discarded.

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use crate::store::{
  lease::LEASE_FILE_NAME,
  share_meta::{read_generation, write_generation},
  sync_copy::{prune_backups, publish_database},
};

const BACKUP_RETENTION: usize = 3;
const GENERATION_SUFFIX: &str = ".generation";
const WAL_SIDECARS: [&str; 2] = ["-wal", "-shm"];

pub fn clean_direct_artifacts(canonical: &Path, working_copy: &Path) {
  remove_database_family(working_copy);
  let _ = fs::remove_file(marker_path(working_copy));
  let _ = fs::remove_file(sidecar_path(canonical));
  if let Some(share) = canonical.parent() {
    let _ = fs::remove_file(share.join(LEASE_FILE_NAME));
  }
}

/// Adopts an existing canonical into the working copy, or resolves divergence.
///
/// When both sides have data but differ in generation, the higher-generation side wins and the
/// loser is backed up via `publish_database`. Markers on both sides are brought to the same
/// generation so the next boot finds no divergence.
pub fn reconcile_sync(canonical: &Path, working_copy: &Path) -> io::Result<()> {
  // Unconditional boot-time prune: self-heals pre-existing backup piles regardless of which branch below fires.
  prune_backups(canonical, BACKUP_RETENTION);
  prune_backups(working_copy, BACKUP_RETENTION);

  let canonical_sidecar = sidecar_path(canonical);
  let wc_marker = marker_path(working_copy);
  let canonical_generation = read_generation(&canonical_sidecar);
  let wc_generation = read_generation(&wc_marker);
  let canonical_has_data = is_non_empty(canonical);
  let wc_has_data = is_non_empty(working_copy);

  if wc_has_data && canonical_has_data && canonical_generation != wc_generation {
    if canonical_generation > wc_generation {
      adopt(canonical, working_copy, true)?;
      write_generation(&wc_marker, canonical_generation)?;
    } else {
      // Advance past whichever side had the higher counter so the merged marker is strictly newer than both.
      let next = wc_generation.max(canonical_generation) + 1;
      adopt(working_copy, canonical, true)?;
      write_generation(&canonical_sidecar, next)?;
      write_generation(&wc_marker, next)?;
    }
  } else if !wc_has_data && canonical_has_data {
    let generation = canonical_generation.max(1);
    adopt(canonical, working_copy, false)?;
    write_generation(&canonical_sidecar, generation)?;
    write_generation(&wc_marker, generation)?;
  } else if wc_has_data && !canonical_has_data {
    let generation = wc_generation.max(canonical_generation).max(1);
    adopt(working_copy, canonical, false)?;
    write_generation(&canonical_sidecar, generation)?;
    write_generation(&wc_marker, generation)?;
  }

  Ok(())
}

/// Copies `source` over `destination`. `back_up` is true only on genuine divergence (both sides
/// held data and lost work would otherwise be overwritten); it is false when the destination is
/// empty and nothing of value is being replaced.
fn adopt(source: &Path, destination: &Path, back_up: bool) -> io::Result<()> {
  publish_database(source, destination, back_up)?;
  // Copy any live WAL/SHM sidecars so an uncheckpointed working-copy WAL is not lost.
  // If the source has no sidecar, remove the destination's to prevent a stale WAL from
  // corrupting the freshly adopted database on next open.
  for suffix in WAL_SIDECARS {
    let source_sidecar = with_suffix(source, suffix);
    let destination_sidecar = with_suffix(destination, suffix);
    if source_sidecar.exists() {
      fs::copy(&source_sidecar, &destination_sidecar)?;
    } else {
      let _ = fs::remove_file(&destination_sidecar);
    }
  }
  Ok(())
}

fn is_non_empty(path: &Path) -> bool {
  fs::metadata(path).is_ok_and(|meta| meta.len() > 0)
}

fn marker_path(working_copy: &Path) -> PathBuf {
  with_suffix(working_copy, GENERATION_SUFFIX)
}

fn remove_database_family(database: &Path) {
  let _ = fs::remove_file(database);
  for suffix in WAL_SIDECARS {
    let _ = fs::remove_file(with_suffix(database, suffix));
  }
}

fn sidecar_path(canonical: &Path) -> PathBuf {
  with_suffix(canonical, GENERATION_SUFFIX)
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
  let mut name = path.as_os_str().to_owned();
  name.push(suffix);
  PathBuf::from(name)
}

#[cfg(test)]
mod tests {
  use tempfile::TempDir;

  use super::*;

  struct Layout {
    _dir: TempDir,
    canonical: PathBuf,
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
        working_copy: local.join("pod.db"),
        _dir: dir,
      }
    }

    fn backup_count(&self, dir: &Path) -> usize {
      fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
        .count()
    }

    fn backup_in(&self, dir: &Path) -> Option<PathBuf> {
      fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
        .map(|entry| entry.path())
    }

    fn seed_backups(&self, database: &Path, stamps: &[&str]) {
      for stamp in stamps {
        let mut name = database.as_os_str().to_owned();
        name.push(format!(".{stamp}.backup"));
        fs::write(PathBuf::from(name), stamp.as_bytes()).unwrap();
      }
    }
  }

  mod reconcile_sync {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_adopts_a_canonical_with_no_sidecar_instead_of_opening_an_empty_working_copy() {
      let layout = Layout::new();
      // The documented gen-0 boot scenario: real canonical data, no .generation sidecar, no working copy.
      fs::write(&layout.canonical, b"real canonical data").unwrap();

      reconcile_sync(&layout.canonical, &layout.working_copy).unwrap();

      assert_eq!(
        fs::read(&layout.working_copy).unwrap(),
        b"real canonical data",
        "the working copy is seeded from the canonical, never opened empty"
      );
      assert_eq!(read_generation(&sidecar_path(&layout.canonical)), 1);
      assert_eq!(
        read_generation(&marker_path(&layout.working_copy)),
        1,
        "the markers are brought in step so the first launch performs no redundant pull"
      );
    }

    #[test]
    fn it_adopts_the_newer_working_copy_and_backs_up_the_diverged_canonical() {
      let layout = Layout::new();
      fs::write(&layout.canonical, b"stale canonical").unwrap();
      fs::write(&layout.working_copy, b"newer working copy").unwrap();
      write_generation(&sidecar_path(&layout.canonical), 3).unwrap();
      write_generation(&marker_path(&layout.working_copy), 8).unwrap();

      reconcile_sync(&layout.canonical, &layout.working_copy).unwrap();

      assert_eq!(
        fs::read(&layout.canonical).unwrap(),
        b"newer working copy",
        "the newer working copy is adopted as truth onto the canonical"
      );
      let backup = layout
        .backup_in(layout.canonical.parent().unwrap())
        .expect("the loser is backed up");
      assert_eq!(fs::read(backup).unwrap(), b"stale canonical");
      assert_eq!(
        read_generation(&sidecar_path(&layout.canonical)),
        read_generation(&marker_path(&layout.working_copy)),
        "markers converge"
      );
    }

    #[test]
    fn it_adopts_the_newer_canonical_and_backs_up_the_diverged_working_copy() {
      let layout = Layout::new();
      fs::write(&layout.canonical, b"newer canonical").unwrap();
      fs::write(&layout.working_copy, b"stale working copy").unwrap();
      write_generation(&sidecar_path(&layout.canonical), 9).unwrap();
      write_generation(&marker_path(&layout.working_copy), 2).unwrap();

      reconcile_sync(&layout.canonical, &layout.working_copy).unwrap();

      assert_eq!(fs::read(&layout.working_copy).unwrap(), b"newer canonical");
      let backup = layout
        .backup_in(layout.working_copy.parent().unwrap())
        .expect("the diverged working copy is backed up");
      assert_eq!(fs::read(backup).unwrap(), b"stale working copy");
      assert_eq!(read_generation(&marker_path(&layout.working_copy)), 9);
    }

    #[test]
    fn it_leaves_a_fresh_install_with_no_data_untouched() {
      let layout = Layout::new();

      reconcile_sync(&layout.canonical, &layout.working_copy).unwrap();

      assert!(
        !layout.working_copy.exists(),
        "no empty database is conjured for a fresh install"
      );
      assert!(!layout.canonical.exists());
    }

    #[test]
    fn it_prunes_pre_existing_backup_piles_beside_both_databases_to_the_newest_three() {
      let layout = Layout::new();
      // In-step markers so no divergence branch fires: the prune must run on its own.
      fs::write(&layout.canonical, b"canonical").unwrap();
      fs::write(&layout.working_copy, b"working copy").unwrap();
      write_generation(&sidecar_path(&layout.canonical), 5).unwrap();
      write_generation(&marker_path(&layout.working_copy), 5).unwrap();
      let stamps = [
        "20260101-000000",
        "20260102-000000",
        "20260103-000000",
        "20260104-000000",
        "20260105-000000",
      ];
      layout.seed_backups(&layout.canonical, &stamps);
      layout.seed_backups(&layout.working_copy, &stamps);

      reconcile_sync(&layout.canonical, &layout.working_copy).unwrap();

      assert_eq!(
        layout.backup_count(layout.canonical.parent().unwrap()),
        3,
        "the canonical's backup pile is pruned to the newest three"
      );
      assert_eq!(
        layout.backup_count(layout.working_copy.parent().unwrap()),
        3,
        "the working copy's backup pile is pruned to the newest three"
      );
    }
  }

  mod clean_direct_artifacts {
    use super::*;

    #[test]
    fn it_removes_stray_sync_artifacts_left_from_a_prior_sync_config() {
      let layout = Layout::new();
      fs::write(&layout.canonical, b"live").unwrap();
      fs::write(&layout.working_copy, b"stray working copy").unwrap();
      fs::write(with_suffix(&layout.working_copy, "-wal"), b"wal").unwrap();
      fs::write(marker_path(&layout.working_copy), b"5").unwrap();
      fs::write(sidecar_path(&layout.canonical), b"5").unwrap();
      let lease = layout.canonical.parent().unwrap().join(LEASE_FILE_NAME);
      fs::write(&lease, b"{}").unwrap();

      clean_direct_artifacts(&layout.canonical, &layout.working_copy);

      assert!(!layout.working_copy.exists(), "the stray working copy is removed");
      assert!(!with_suffix(&layout.working_copy, "-wal").exists());
      assert!(!marker_path(&layout.working_copy).exists());
      assert!(!sidecar_path(&layout.canonical).exists());
      assert!(!lease.exists(), "the share lease is removed");
      assert!(layout.canonical.exists(), "the canonical database is left intact");
    }
  }
}
