//! Age-based sweep of the two file sets Pod grows without bound: daily log files and timestamped
//! database backups. Both are swept once at startup, before the log appender rotates and without
//! waiting on any sync activity, so a Pod that sits closed for a week still cleans up on the next
//! launch.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, NaiveDate, Utc};

use crate::{features::settings::log_export, store::sync_copy};

pub(super) const RETENTION_DAYS: i64 = 7;

pub(super) fn sweep(log_dir: &Path, databases: &[PathBuf]) {
  let now = Utc::now();
  sweep_logs(log_dir, now);
  for database in databases {
    sync_copy::prune_backups_older_than(database, now - Duration::days(RETENTION_DAYS));
  }
}

/// Deletes `pod.YYYY-MM-DD.log` files dated before the retention window. Anything else in the
/// directory is left alone, and the cutoff day itself survives so the Last 7 days export still has
/// a full seven days to read.
fn sweep_logs(log_dir: &Path, now: DateTime<Utc>) {
  let cutoff = (now - Duration::days(RETENTION_DAYS)).date_naive();
  let Ok(entries) = std::fs::read_dir(log_dir) else {
    return;
  };

  for entry in entries.flatten() {
    let name = entry.file_name().to_string_lossy().into_owned();
    let Some(day) = log_export::day_of_file(&name) else {
      continue;
    };
    if day >= cutoff {
      continue;
    }
    remove_log(&entry.path(), day);
  }
}

fn remove_log(path: &Path, day: NaiveDate) {
  if let Err(error) = std::fs::remove_file(path) {
    tracing::warn!(
      target: "pod::lifecycle",
      path = %path.display(),
      %day,
      %error,
      "could not delete a stale log file",
    );
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod sweep {
    use super::*;

    #[test]
    fn it_returns_without_panicking_for_an_unreadable_directory() {
      let dir = tempfile::tempdir().expect("temp dir");
      let missing = dir.path().join("no-such-dir");

      sweep(&missing, &[missing.join("pod.db")]);
    }
  }

  mod sweep_logs {
    use std::io::Read as _;

    use chrono::TimeZone as _;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::features::settings::log_export::Diagnostics;

    fn now() -> DateTime<Utc> {
      Utc.with_ymd_and_hms(2026, 8, 16, 12, 0, 0).unwrap()
    }

    fn write(dir: &Path, name: &str) -> PathBuf {
      let path = dir.join(name);
      std::fs::write(&path, b"{}\n").expect("write log");
      path
    }

    #[test]
    fn it_deletes_a_file_one_day_past_the_cutoff() {
      let dir = tempfile::tempdir().expect("temp dir");
      let stale = write(dir.path(), "pod.2026-08-08.log");

      sweep_logs(dir.path(), now());

      assert!(!stale.exists());
    }

    #[test]
    fn it_ignores_files_it_does_not_own() {
      let dir = tempfile::tempdir().expect("temp dir");
      let foreign = write(dir.path(), "notes.txt");
      let mislabelled = write(dir.path(), "pod.not-a-date.log");

      sweep_logs(dir.path(), now());

      assert!(foreign.exists());
      assert!(mislabelled.exists());
    }

    #[test]
    fn it_keeps_a_file_one_day_inside_the_cutoff() {
      let dir = tempfile::tempdir().expect("temp dir");
      let fresh = write(dir.path(), "pod.2026-08-09.log");

      sweep_logs(dir.path(), now());

      assert!(fresh.exists());
    }

    #[test]
    fn it_keeps_a_full_seven_days_including_the_file_being_written() {
      let dir = tempfile::tempdir().expect("temp dir");
      let days: Vec<PathBuf> = (9..=16)
        .map(|day| write(dir.path(), &format!("pod.2026-08-{day:02}.log")))
        .collect();
      let stale = write(dir.path(), "pod.2026-08-08.log");

      sweep_logs(dir.path(), now());

      assert_eq!(days.iter().filter(|path| path.exists()).count(), 8);
      assert!(!stale.exists());
    }

    #[test]
    fn it_leaves_the_last_seven_days_export_a_complete_set() {
      let dir = tempfile::tempdir().expect("temp dir");
      let now = now();
      for day in 7..=16 {
        let name = format!("pod.2026-08-{day:02}.log");
        let entry = |hour: u32| {
          format!("{{\"timestamp\":\"2026-08-{day:02}T{hour:02}:00:00Z\",\"fields\":{{\"message\":\"x\"}}}}\n")
        };
        std::fs::write(dir.path().join(name), entry(6) + &entry(18)).expect("write log");
      }

      sweep_logs(dir.path(), now);
      let zip = log_export::build_zip(
        dir.path(),
        now - Duration::days(RETENTION_DAYS),
        now,
        &Diagnostics {
          cache_dir: PathBuf::from("/cache"),
          database_path: PathBuf::from("/db/pod.db"),
          db_dir: PathBuf::from("/db"),
          log_dir: dir.path().to_path_buf(),
        },
      )
      .expect("build zip");

      let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).expect("read zip");
      let mut names = Vec::new();
      for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("entry");
        let mut contents = String::new();
        entry.read_to_string(&mut contents).expect("read entry");
        names.push(entry.name().to_owned());
      }
      names.sort();
      names.retain(|name| name.ends_with(".log"));

      assert_eq!(
        names,
        (9..=16)
          .map(|day| format!("pod.2026-08-{day:02}.log"))
          .collect::<Vec<_>>(),
        "the sweep spares every day the seven-day export reads"
      );
    }
  }
}
