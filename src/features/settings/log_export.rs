use std::{
  io::{BufRead, BufReader, Cursor, Write},
  path::{Path, PathBuf},
};

use chrono::{DateTime, Duration, Local, TimeZone, Utc};
use zip::{CompressionMethod, ZipWriter, write::FileOptions};

const DATE_FORMAT: &str = "%Y-%m-%d";
const FILE_PREFIX: &str = "pod.";
const FILE_SUFFIX: &str = ".log";
const MANIFEST_NAME: &str = "MANIFEST.txt";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostics {
  pub cache_dir: PathBuf,
  pub database_path: PathBuf,
  pub db_dir: PathBuf,
  pub log_dir: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RangePreset {
  Last24Hours,
  Last7Days,
  LastHour,
  Today,
}

impl RangePreset {
  pub fn label(self) -> &'static str {
    let key = match self {
      RangePreset::Last24Hours => "settings.log_export.last_24h",
      RangePreset::Last7Days => "settings.log_export.last_7_days",
      RangePreset::LastHour => "settings.log_export.last_hour",
      RangePreset::Today => "settings.log_export.today",
    };
    super::i18n::tr_static(key)
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IncludedFile {
  bytes: u64,
  lines: u64,
  name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedFile {
  boundary: bool,
  name: String,
  path: PathBuf,
}

pub fn build_zip(
  log_dir: &Path,
  start: DateTime<Utc>,
  end: DateTime<Utc>,
  diagnostics: &Diagnostics,
) -> Result<Vec<u8>, String> {
  let selected = select_files(log_dir, start, end);

  let mut buf = Vec::new();
  {
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let options: FileOptions<'_, ()> = FileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut included = Vec::new();
    for file in &selected {
      zip
        .start_file(&file.name, options)
        .map_err(|err| t!("settings.log_export.error_add_file", name => file.name, error => err).into_owned())?;
      let summary = stream_lines(&mut zip, file, start, end)?;
      included.push(summary);
    }

    zip
      .start_file(MANIFEST_NAME, options)
      .map_err(|err| t!("settings.log_export.error_add_manifest", error => err).into_owned())?;
    let manifest = render_manifest(start, end, diagnostics, &included);
    zip
      .write_all(manifest.as_bytes())
      .map_err(|err| t!("settings.log_export.error_write_manifest", error => err).into_owned())?;

    zip
      .finish()
      .map_err(|err| t!("settings.log_export.error_finalize", error => err).into_owned())?;
  }
  Ok(buf)
}

pub fn default_file_name(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
  format!(
    "pod-logs-{}-{}.zip",
    start.format("%Y%m%dT%H%M%SZ"),
    end.format("%Y%m%dT%H%M%SZ")
  )
}

pub fn range_for_preset(preset: RangePreset, now: DateTime<Local>) -> (DateTime<Utc>, DateTime<Utc>) {
  let start = match preset {
    RangePreset::Last24Hours => now - Duration::hours(24),
    RangePreset::Last7Days => now - Duration::days(7),
    RangePreset::LastHour => now - Duration::hours(1),
    RangePreset::Today => now
      .date_naive()
      .and_hms_opt(0, 0, 0)
      .and_then(|midnight| Local.from_local_datetime(&midnight).single())
      .unwrap_or(now),
  };
  (start.with_timezone(&Utc), now.with_timezone(&Utc))
}

fn line_timestamp(line: &str) -> Option<DateTime<Utc>> {
  let value: serde_json::Value = serde_json::from_str(line).ok()?;
  let raw = value.get("timestamp")?.as_str()?;
  DateTime::parse_from_rfc3339(raw).ok().map(|ts| ts.with_timezone(&Utc))
}

fn render_manifest(
  start: DateTime<Utc>,
  end: DateTime<Utc>,
  diagnostics: &Diagnostics,
  included: &[IncludedFile],
) -> String {
  let mut out = String::new();
  out.push_str("Pod log export\n");
  out.push_str(&format!("Pod version: {}\n", env!("CARGO_PKG_VERSION")));
  out.push_str(&format!(
    "OS/arch: {}/{}\n",
    std::env::consts::OS,
    std::env::consts::ARCH
  ));
  out.push_str("Timezone: presets are interpreted in local time and converted to UTC for filtering\n");
  out.push_str(&format!(
    "Range (UTC): {} .. {}\n",
    start.to_rfc3339(),
    end.to_rfc3339()
  ));
  out.push_str("\nStorage paths:\n");
  out.push_str(&format!("  database: {}\n", diagnostics.database_path.display()));
  out.push_str(&format!("  db dir:   {}\n", diagnostics.db_dir.display()));
  out.push_str(&format!("  cache:    {}\n", diagnostics.cache_dir.display()));
  out.push_str(&format!("  logs:     {}\n", diagnostics.log_dir.display()));
  out.push_str("\nIncluded files:\n");
  if included.is_empty() {
    out.push_str("  (none: no log lines in range)\n");
  }
  for file in included {
    out.push_str(&format!(
      "  {} ({} lines, {} bytes)\n",
      file.name, file.lines, file.bytes
    ));
  }
  out
}

fn select_files(log_dir: &Path, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<SelectedFile> {
  let Ok(entries) = std::fs::read_dir(log_dir) else {
    return Vec::new();
  };

  let mut selected = Vec::new();
  for entry in entries.flatten() {
    let name = entry.file_name().to_string_lossy().into_owned();
    let Some(day) = day_of_file(&name) else {
      continue;
    };
    let day_start = Utc.from_utc_datetime(&day.and_hms_opt(0, 0, 0).unwrap());
    let day_end = day_start + Duration::days(1);

    if day_end <= start || day_start >= end {
      continue;
    }
    selected.push(SelectedFile {
      boundary: day_start < start || day_end > end,
      name,
      path: entry.path(),
    });
  }

  selected.sort_by(|a, b| a.name.cmp(&b.name));
  selected
}

/// Parses the day out of a `pod.YYYY-MM-DD.log` name, returning `None` for any file the appender
/// did not write. The startup retention sweep shares this so it and the export agree on exactly
/// which files are Pod's.
pub fn day_of_file(name: &str) -> Option<chrono::NaiveDate> {
  let date = name.strip_prefix(FILE_PREFIX)?.strip_suffix(FILE_SUFFIX)?;
  chrono::NaiveDate::parse_from_str(date, DATE_FORMAT).ok()
}

fn stream_lines<W: Write + std::io::Seek>(
  zip: &mut ZipWriter<W>,
  file: &SelectedFile,
  start: DateTime<Utc>,
  end: DateTime<Utc>,
) -> Result<IncludedFile, String> {
  let handle = std::fs::File::open(&file.path)
    .map_err(|err| t!("settings.log_export.error_read_file", name => file.name, error => err).into_owned())?;
  let reader = BufReader::new(handle);

  let mut lines = 0;
  let mut bytes = 0;
  for line in reader.lines() {
    let line =
      line.map_err(|err| t!("settings.log_export.error_read_file", name => file.name, error => err).into_owned())?;
    if file.boundary {
      match line_timestamp(&line) {
        Some(ts) if ts >= start && ts < end => {}
        _ => continue,
      }
    }
    zip
      .write_all(line.as_bytes())
      .and_then(|()| zip.write_all(b"\n"))
      .map_err(|err| t!("settings.log_export.error_write_file", name => file.name, error => err).into_owned())?;
    lines += 1;
    bytes += line.len() as u64 + 1;
  }

  Ok(IncludedFile {
    bytes,
    lines,
    name: file.name.clone(),
  })
}

#[cfg(test)]
mod tests {
  use std::io::Read;

  use chrono::TimeZone;

  use super::*;

  fn utc(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, 0, 0).unwrap()
  }

  fn write_log(dir: &Path, name: &str, lines: &[&str]) {
    std::fs::write(dir.join(name), format!("{}\n", lines.join("\n"))).unwrap();
  }

  fn line_at(ts: &str) -> String {
    format!("{{\"timestamp\":\"{ts}\",\"level\":\"INFO\",\"fields\":{{\"message\":\"x\"}}}}")
  }

  fn read_entries(zip: &[u8]) -> std::collections::HashMap<String, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip)).unwrap();
    let mut out = std::collections::HashMap::new();
    for i in 0..archive.len() {
      let mut entry = archive.by_index(i).unwrap();
      let mut contents = String::new();
      entry.read_to_string(&mut contents).unwrap();
      out.insert(entry.name().to_owned(), contents);
    }
    out
  }

  fn diagnostics() -> Diagnostics {
    Diagnostics {
      cache_dir: PathBuf::from("/cache"),
      database_path: PathBuf::from("/db/pod.db"),
      db_dir: PathBuf::from("/db"),
      log_dir: PathBuf::from("/logs"),
    }
  }

  mod build_zip {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_includes_interior_files_whole_filters_boundary_days_and_skips_the_rest() {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path();
      write_log(
        path,
        "pod.2026-06-09.log",
        &[
          &line_at("2026-06-09T05:00:00Z"),
          &line_at("2026-06-09T07:00:00Z"),
          &line_at("2026-06-09T08:00:00Z"),
        ],
      );
      write_log(
        path,
        "pod.2026-06-10.log",
        &[&line_at("2026-06-10T01:00:00Z"), &line_at("2026-06-10T23:00:00Z")],
      );
      write_log(path, "pod.2026-06-08.log", &[&line_at("2026-06-08T12:00:00Z")]);
      write_log(path, "pod.2026-06-11.log", &[&line_at("2026-06-11T01:00:00Z")]);

      let bytes = build_zip(path, utc(2026, 6, 9, 6), utc(2026, 6, 11, 0), &diagnostics()).unwrap();
      let entries = read_entries(&bytes);

      assert_eq!(entries.len(), 3, "two log files plus the manifest");
      assert_eq!(
        entries["pod.2026-06-09.log"].lines().count(),
        2,
        "boundary day keeps only in-range lines"
      );
      assert_eq!(
        entries["pod.2026-06-10.log"].lines().count(),
        2,
        "interior day is included whole"
      );
      assert!(!entries.contains_key("pod.2026-06-08.log"));
      assert!(!entries.contains_key("pod.2026-06-11.log"));
    }

    #[test]
    fn the_manifest_records_the_range_and_included_files() {
      let dir = tempfile::tempdir().unwrap();
      write_log(dir.path(), "pod.2026-06-10.log", &[&line_at("2026-06-10T01:00:00Z")]);

      let bytes = build_zip(dir.path(), utc(2026, 6, 10, 0), utc(2026, 6, 11, 0), &diagnostics()).unwrap();
      let entries = read_entries(&bytes);
      let manifest = &entries[MANIFEST_NAME];

      assert!(manifest.contains("Range (UTC): 2026-06-10T00:00:00+00:00 .. 2026-06-11T00:00:00+00:00"));
      assert!(manifest.contains("pod.2026-06-10.log (1 lines"));
      assert!(manifest.contains("/db/pod.db"));
    }
  }

  mod default_file_name {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_brackets_the_range_in_the_name() {
      let name = default_file_name(utc(2026, 6, 9, 6), utc(2026, 6, 11, 0));

      assert_eq!(name, "pod-logs-20260609T060000Z-20260611T000000Z.zip");
    }
  }

  mod range_for_preset {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_spans_a_rolling_hour_for_last_hour() {
      let now = Local.with_ymd_and_hms(2026, 6, 10, 15, 30, 0).unwrap();

      let (start, end) = range_for_preset(RangePreset::LastHour, now);

      assert_eq!(end - start, Duration::hours(1));
    }

    #[test]
    fn it_spans_a_rolling_week_for_last_seven_days() {
      let now = Local.with_ymd_and_hms(2026, 6, 10, 15, 30, 0).unwrap();

      let (start, end) = range_for_preset(RangePreset::Last7Days, now);

      assert_eq!(end - start, Duration::days(7));
    }

    #[test]
    fn it_starts_at_local_midnight_for_today() {
      let now = Local.with_ymd_and_hms(2026, 6, 10, 15, 30, 0).unwrap();

      let (start, end) = range_for_preset(RangePreset::Today, now);

      assert_eq!(end - start, Duration::hours(15) + Duration::minutes(30));
    }
  }
}
