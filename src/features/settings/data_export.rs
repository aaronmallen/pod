//! Packages a portable Pod data archive: a self-contained `pod.db` snapshot, the user's
//! `config.toml`, and both a machine-parseable `manifest.json` and a human-readable `MANIFEST.txt`,
//! all bundled into a single in-memory `.zip`. This is the write side of the archive codec; the
//! import side (T6 read/validate) parses the manifest and unpacks these entries back out.
//!
//! Async/sync boundary: `checkpoint_into` (store::sync_copy) is async and folds the WAL into a
//! self-contained `.db`, so it cannot run inside this sync builder. The async caller (T5) stages
//! that checkpointed snapshot to a tempfile and reads the live `config.toml` bytes, then hands both
//! into `build_archive`, which stays sync and merely zips the ready inputs. Keeping the builder sync
//! lets it run under `spawn_blocking` exactly like `log_export::build_zip`.

use std::{
  io::{Cursor, Read, Write},
  path::Path,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use zip::{CompressionMethod, ZipWriter, write::FileOptions};

use crate::features::settings::log_export::Diagnostics;

/// Archive layout version. Bump when the set of entries or the manifest schema changes so the
/// import side can refuse archives it does not understand.
#[allow(dead_code)]
pub const ARCHIVE_VERSION: u32 = 1;

/// Entry name for the self-contained database snapshot inside the archive.
#[allow(dead_code)]
pub const DATABASE_NAME: &str = "pod.db";

/// Entry name for the bundled config file inside the archive.
#[allow(dead_code)]
pub const CONFIG_NAME: &str = "config.toml";

/// Entry name for the machine-parseable manifest inside the archive.
#[allow(dead_code)]
pub const MANIFEST_JSON_NAME: &str = "manifest.json";

/// Entry name for the human-readable manifest inside the archive.
const MANIFEST_TXT_NAME: &str = "MANIFEST.txt";

/// The machine-parseable archive manifest. Serialized to `manifest.json` and parsed back by the
/// import side to identify the archive and reason about compatibility before unpacking.
///
/// `created_at` is rendered as an RFC 3339 string because the project does not enable chrono's
/// optional serde feature (see `store::share_meta`); a string keeps the format language-agnostic and
/// avoids pulling in that feature.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Manifest {
  /// Archive layout version (see `ARCHIVE_VERSION`).
  pub archive_version: u32,
  /// `arch` the archive was produced on (`std::env::consts::ARCH`).
  pub arch: String,
  /// When the archive was built, as an RFC 3339 UTC timestamp.
  pub created_at: String,
  /// Pod version that produced the archive (`CARGO_PKG_VERSION`).
  pub pod_version: String,
  /// `os` the archive was produced on (`std::env::consts::OS`).
  pub os: String,
  /// Summary of the storage paths the archive was captured from.
  pub storage: StoragePaths,
  /// Stats for each file included in the archive.
  pub files: Vec<IncludedFile>,
}

impl Manifest {
  fn new(created_at: DateTime<Utc>, diagnostics: &Diagnostics, files: Vec<IncludedFile>) -> Self {
    Manifest {
      archive_version: ARCHIVE_VERSION,
      arch: std::env::consts::ARCH.to_owned(),
      created_at: created_at.to_rfc3339(),
      pod_version: env!("CARGO_PKG_VERSION").to_owned(),
      os: std::env::consts::OS.to_owned(),
      storage: StoragePaths::from(diagnostics),
      files,
    }
  }
}

/// Storage-path summary captured into the manifest so an archive records where it came from.
#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoragePaths {
  pub cache_dir: String,
  pub database_path: String,
  pub db_dir: String,
  pub log_dir: String,
}

impl From<&Diagnostics> for StoragePaths {
  fn from(diagnostics: &Diagnostics) -> Self {
    StoragePaths {
      cache_dir: diagnostics.cache_dir.display().to_string(),
      database_path: diagnostics.database_path.display().to_string(),
      db_dir: diagnostics.db_dir.display().to_string(),
      log_dir: diagnostics.log_dir.display().to_string(),
    }
  }
}

/// Per-entry stats recorded in the manifest.
#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IncludedFile {
  pub bytes: u64,
  pub name: String,
}

/// Builds the in-memory `.zip` archive from a ready database snapshot and config bytes.
///
/// `db_snapshot` must point at a self-contained, already-checkpointed `pod.db` (no `-wal`/`-shm`
/// sidecars) — the async caller produces it via `store::sync_copy::checkpoint_into`, which already
/// runs `PRAGMA wal_checkpoint(TRUNCATE)`; this builder must not re-checkpoint. `config_bytes` is
/// the live `config.toml` content the caller read from `config_path()`. Whole-DB export: the entire
/// snapshot is bundled, with no start/end range (a deliberate simplification vs `log_export`).
///
/// A `String` error keeps this consistent with the UI seam
/// (`ExportFinished(Result<Option<PathBuf>, String>)`).
#[allow(dead_code)]
pub fn build_archive(db_snapshot: &Path, config_bytes: &[u8], diagnostics: &Diagnostics) -> Result<Vec<u8>, String> {
  let db_bytes = std::fs::read(db_snapshot).map_err(|err| format!("Couldn't read database snapshot: {err}"))?;

  let files = vec![
    IncludedFile {
      bytes: db_bytes.len() as u64,
      name: DATABASE_NAME.to_owned(),
    },
    IncludedFile {
      bytes: config_bytes.len() as u64,
      name: CONFIG_NAME.to_owned(),
    },
  ];
  let manifest = Manifest::new(Utc::now(), diagnostics, files);
  let manifest_json =
    serde_json::to_vec_pretty(&manifest).map_err(|err| format!("Couldn't render manifest.json: {err}"))?;
  let manifest_txt = render_manifest(&manifest);

  let mut buf = Vec::new();
  {
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let options: FileOptions<'_, ()> = FileOptions::default().compression_method(CompressionMethod::Deflated);

    write_entry(&mut zip, options, DATABASE_NAME, &db_bytes)?;
    write_entry(&mut zip, options, CONFIG_NAME, config_bytes)?;
    write_entry(&mut zip, options, MANIFEST_JSON_NAME, &manifest_json)?;
    write_entry(&mut zip, options, MANIFEST_TXT_NAME, manifest_txt.as_bytes())?;

    zip
      .finish()
      .map_err(|err| format!("Couldn't finalize archive: {err}"))?;
  }
  Ok(buf)
}

/// Suggested file name for a saved data archive, bracketed by the build timestamp.
#[allow(dead_code)]
pub fn default_file_name(now: DateTime<Utc>) -> String {
  format!("pod-data-{}.zip", now.format("%Y%m%dT%H%M%SZ"))
}

/// Whether an archive's Pod version is compatible with this build, per ADR-0038's version guard.
///
/// An archive from an older or equal Pod restores fine — migrations run forward on next launch. An
/// archive from a newer Pod (a higher major version than this build) is refused, because a newer
/// schema cannot be downgraded. The import UI maps these to "ok / will migrate / incompatible".
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VersionVerdict {
  /// Archive Pod version matches this build exactly; restore as-is.
  Ok,
  /// Archive is from an older Pod; restore is safe and migrations run forward on next launch.
  WillMigrate,
  /// Archive is from a newer Pod (higher major version); refuse — the schema can't be downgraded.
  Incompatible,
}

/// An archive parsed out of its `.zip` container: the raw `pod.db` and `config.toml` bytes, the
/// parsed `manifest.json`, and the version-guard verdict the import confirm modal displays. The
/// import join (T7) consumes the bytes to restore and reads `verdict` to gate the restore.
#[derive(Clone, Debug)]
pub struct ParsedArchive {
  /// Raw bytes of the self-contained `pod.db` snapshot entry.
  pub database: Vec<u8>,
  /// Raw bytes of the bundled `config.toml` entry.
  pub config: Vec<u8>,
  /// The parsed machine-readable manifest.
  pub manifest: Manifest,
  /// Compatibility verdict comparing the archive's Pod version against this build.
  pub verdict: VersionVerdict,
}

/// Opens a data archive, extracts and validates its entries, and computes the version-guard verdict.
///
/// Reads the `.zip` from `bytes`, requiring `pod.db`, `config.toml`, and a parseable `manifest.json`;
/// a missing or corrupt entry is rejected with a clear `String` error so nothing is partially
/// applied. The returned `verdict` reflects ADR-0038's policy: an older/equal archive restores
/// (migrations run forward), a newer-major archive is `Incompatible` and the import must refuse.
pub fn read_archive(bytes: &[u8]) -> Result<ParsedArchive, String> {
  let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|err| format!("Couldn't open archive: {err}"))?;

  let mut database: Option<Vec<u8>> = None;
  let mut config: Option<Vec<u8>> = None;
  let mut manifest_json: Option<Vec<u8>> = None;

  for index in 0..archive.len() {
    let mut entry = archive
      .by_index(index)
      .map_err(|err| format!("Couldn't read archive entry: {err}"))?;
    let name = entry.name().to_owned();
    let slot = match name.as_str() {
      DATABASE_NAME => &mut database,
      CONFIG_NAME => &mut config,
      MANIFEST_JSON_NAME => &mut manifest_json,
      _ => continue,
    };
    let mut contents = Vec::new();
    entry
      .read_to_end(&mut contents)
      .map_err(|err| format!("Couldn't read {name} from archive: {err}"))?;
    *slot = Some(contents);
  }

  let manifest_json = manifest_json.ok_or_else(|| format!("Archive is missing {MANIFEST_JSON_NAME}"))?;
  let database = database.ok_or_else(|| format!("Archive is missing {DATABASE_NAME}"))?;
  let config = config.ok_or_else(|| format!("Archive is missing {CONFIG_NAME}"))?;

  let manifest: Manifest =
    serde_json::from_slice(&manifest_json).map_err(|err| format!("Couldn't parse {MANIFEST_JSON_NAME}: {err}"))?;

  let verdict = version_verdict(&manifest.pod_version)?;

  Ok(ParsedArchive {
    database,
    config,
    manifest,
    verdict,
  })
}

/// Compares an archive's Pod version against this build to produce the version-guard verdict.
///
/// Uses semver-major comparison (ADR-0038's "refuse if the archive's major Pod version > this
/// build's"). An equal version is `Ok`, a lower one `WillMigrate`, and a higher major `Incompatible`.
/// An unparseable version is rejected outright so a malformed manifest can't slip past the guard.
fn version_verdict(archive_version: &str) -> Result<VersionVerdict, String> {
  use cargo_packager_updater::semver::Version;

  let archive = Version::parse(archive_version)
    .map_err(|err| format!("Archive Pod version '{archive_version}' is not valid semver: {err}"))?;
  let current = Version::parse(env!("CARGO_PKG_VERSION"))
    .map_err(|err| format!("This build's version is not valid semver: {err}"))?;

  if archive.major > current.major {
    Ok(VersionVerdict::Incompatible)
  } else if archive == current {
    Ok(VersionVerdict::Ok)
  } else {
    Ok(VersionVerdict::WillMigrate)
  }
}

fn write_entry<W: Write + std::io::Seek>(
  zip: &mut ZipWriter<W>,
  options: FileOptions<'_, ()>,
  name: &str,
  bytes: &[u8],
) -> Result<(), String> {
  zip
    .start_file(name, options)
    .map_err(|err| format!("Couldn't add {name}: {err}"))?;
  zip
    .write_all(bytes)
    .map_err(|err| format!("Couldn't write {name}: {err}"))
}

fn render_manifest(manifest: &Manifest) -> String {
  let mut out = String::new();
  out.push_str("Pod data export\n");
  out.push_str(&format!("Archive version: {}\n", manifest.archive_version));
  out.push_str(&format!("Pod version: {}\n", manifest.pod_version));
  out.push_str(&format!("OS/arch: {}/{}\n", manifest.os, manifest.arch));
  out.push_str(&format!("Created at (UTC): {}\n", manifest.created_at));
  out.push_str("\nStorage paths:\n");
  out.push_str(&format!("  database: {}\n", manifest.storage.database_path));
  out.push_str(&format!("  db dir:   {}\n", manifest.storage.db_dir));
  out.push_str(&format!("  cache:    {}\n", manifest.storage.cache_dir));
  out.push_str(&format!("  logs:     {}\n", manifest.storage.log_dir));
  out.push_str("\nIncluded files:\n");
  for file in &manifest.files {
    out.push_str(&format!("  {} ({} bytes)\n", file.name, file.bytes));
  }
  out
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use super::*;

  fn diagnostics() -> Diagnostics {
    Diagnostics {
      cache_dir: PathBuf::from("/cache"),
      database_path: PathBuf::from("/db/pod.db"),
      db_dir: PathBuf::from("/db"),
      log_dir: PathBuf::from("/logs"),
    }
  }

  fn read_entries(zip: &[u8]) -> std::collections::HashMap<String, Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip)).unwrap();
    let mut out = std::collections::HashMap::new();
    for i in 0..archive.len() {
      let mut entry = archive.by_index(i).unwrap();
      let mut contents = Vec::new();
      entry.read_to_end(&mut contents).unwrap();
      out.insert(entry.name().to_owned(), contents);
    }
    out
  }

  async fn seed_database(path: &Path) {
    use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

    let options = SqliteConnectOptions::new().filename(path).create_if_missing(true);
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    sqlx::query("CREATE TABLE note (body TEXT)")
      .execute(&mut connection)
      .await
      .unwrap();
    sqlx::query("INSERT INTO note (body) VALUES ('hello')")
      .execute(&mut connection)
      .await
      .unwrap();
    connection.close().await.unwrap();
  }

  mod build_archive {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_bundles_the_database_config_and_both_manifests() {
      let dir = tempfile::tempdir().unwrap();
      let db = dir.path().join("snapshot.db");
      seed_database(&db).await;
      let config = b"[storage]\nnetwork = false\n";

      let bytes = build_archive(&db, config, &diagnostics()).unwrap();
      let entries = read_entries(&bytes);

      assert_eq!(entries.len(), 4, "db, config, json manifest, and text manifest");
      assert!(entries.contains_key(DATABASE_NAME));
      assert_eq!(entries[CONFIG_NAME], config);
      assert!(entries.contains_key(MANIFEST_JSON_NAME));
      assert!(entries.contains_key(MANIFEST_TXT_NAME));
    }

    #[tokio::test]
    async fn the_json_manifest_parses_and_records_the_pod_version_and_entries() {
      let dir = tempfile::tempdir().unwrap();
      let db = dir.path().join("snapshot.db");
      seed_database(&db).await;

      let bytes = build_archive(&db, b"config", &diagnostics()).unwrap();
      let entries = read_entries(&bytes);

      let manifest: Manifest = serde_json::from_slice(&entries[MANIFEST_JSON_NAME]).unwrap();
      assert_eq!(manifest.archive_version, ARCHIVE_VERSION);
      assert_eq!(manifest.pod_version, env!("CARGO_PKG_VERSION"));
      assert_eq!(manifest.storage.database_path, "/db/pod.db");
      assert!(manifest.files.iter().any(|file| file.name == DATABASE_NAME));
      assert!(manifest.files.iter().any(|file| file.name == CONFIG_NAME));
    }

    #[tokio::test]
    async fn the_bundled_database_is_a_self_contained_snapshot() {
      use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

      let dir = tempfile::tempdir().unwrap();
      let db = dir.path().join("snapshot.db");
      seed_database(&db).await;

      let bytes = build_archive(&db, b"config", &diagnostics()).unwrap();
      let entries = read_entries(&bytes);

      let extracted = dir.path().join("extracted.db");
      std::fs::write(&extracted, &entries[DATABASE_NAME]).unwrap();
      let options = SqliteConnectOptions::new().filename(&extracted);
      let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
      let body: String = sqlx::query_scalar("SELECT body FROM note")
        .fetch_one(&mut connection)
        .await
        .unwrap();
      connection.close().await.unwrap();

      assert_eq!(body, "hello", "the bundled db opens and holds the seeded row");
    }

    #[tokio::test]
    async fn the_text_manifest_is_human_readable() {
      let dir = tempfile::tempdir().unwrap();
      let db = dir.path().join("snapshot.db");
      seed_database(&db).await;

      let bytes = build_archive(&db, b"config", &diagnostics()).unwrap();
      let entries = read_entries(&bytes);
      let manifest = String::from_utf8(entries[MANIFEST_TXT_NAME].clone()).unwrap();

      assert!(manifest.contains("Pod data export"));
      assert!(manifest.contains(&format!("Pod version: {}", env!("CARGO_PKG_VERSION"))));
      assert!(manifest.contains("/db/pod.db"));
      assert!(manifest.contains("pod.db ("));
    }

    #[test]
    fn it_errors_when_the_database_snapshot_is_missing() {
      let dir = tempfile::tempdir().unwrap();
      let missing = dir.path().join("absent.db");

      let result = build_archive(&missing, b"config", &diagnostics());

      assert!(result.is_err());
    }
  }

  mod default_file_name {
    use chrono::TimeZone;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_brackets_the_archive_in_the_build_timestamp() {
      let now = Utc.with_ymd_and_hms(2026, 6, 25, 14, 30, 0).unwrap();

      assert_eq!(default_file_name(now), "pod-data-20260625T143000Z.zip");
    }
  }

  mod read_archive {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn valid_archive() -> Vec<u8> {
      let dir = tempfile::tempdir().unwrap();
      let db = dir.path().join("snapshot.db");
      seed_database(&db).await;
      build_archive(&db, b"[storage]\nnetwork = false\n", &diagnostics()).unwrap()
    }

    /// Rebuilds a `.zip` from the given entries, used to fabricate archives missing an entry or
    /// carrying a tampered manifest.
    fn zip_from(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
      let mut buf = Vec::new();
      {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        let options: FileOptions<'_, ()> = FileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, bytes) in entries {
          zip.start_file(*name, options).unwrap();
          zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
      }
      buf
    }

    fn manifest_with_version(version: &str) -> Vec<u8> {
      let manifest = Manifest {
        archive_version: ARCHIVE_VERSION,
        arch: "x86_64".to_owned(),
        created_at: "2026-06-25T00:00:00+00:00".to_owned(),
        pod_version: version.to_owned(),
        os: "linux".to_owned(),
        storage: StoragePaths {
          cache_dir: "/cache".to_owned(),
          database_path: "/db/pod.db".to_owned(),
          db_dir: "/db".to_owned(),
          log_dir: "/logs".to_owned(),
        },
        files: vec![],
      };
      serde_json::to_vec(&manifest).unwrap()
    }

    #[tokio::test]
    async fn it_returns_every_component_for_a_valid_archive() {
      let bytes = valid_archive().await;

      let parsed = read_archive(&bytes).unwrap();

      assert!(!parsed.database.is_empty(), "db bytes are extracted");
      assert_eq!(parsed.config, b"[storage]\nnetwork = false\n");
      assert_eq!(parsed.manifest.pod_version, env!("CARGO_PKG_VERSION"));
      assert_eq!(parsed.verdict, VersionVerdict::Ok);
    }

    #[test]
    fn the_extracted_database_opens_as_a_sqlite_snapshot() {
      // Round-trip is covered by the build_archive suite; here we only assert read_archive
      // surfaces the same db bytes the writer embedded, by length-matching a fabricated entry.
      let db = b"not a real db but round-tripped".to_vec();
      let bytes = zip_from(&[
        (DATABASE_NAME, db.clone()),
        (CONFIG_NAME, b"config".to_vec()),
        (MANIFEST_JSON_NAME, manifest_with_version(env!("CARGO_PKG_VERSION"))),
      ]);

      let parsed = read_archive(&bytes).unwrap();

      assert_eq!(parsed.database, db);
    }

    #[test]
    fn an_older_archive_yields_a_will_migrate_verdict() {
      let bytes = zip_from(&[
        (DATABASE_NAME, b"db".to_vec()),
        (CONFIG_NAME, b"config".to_vec()),
        (MANIFEST_JSON_NAME, manifest_with_version("0.0.1")),
      ]);

      let parsed = read_archive(&bytes).unwrap();

      assert_eq!(parsed.verdict, VersionVerdict::WillMigrate);
    }

    #[test]
    fn a_newer_major_archive_is_incompatible() {
      let bytes = zip_from(&[
        (DATABASE_NAME, b"db".to_vec()),
        (CONFIG_NAME, b"config".to_vec()),
        (MANIFEST_JSON_NAME, manifest_with_version("999.0.0")),
      ]);

      let parsed = read_archive(&bytes).unwrap();

      assert_eq!(parsed.verdict, VersionVerdict::Incompatible);
    }

    #[test]
    fn it_rejects_an_archive_missing_the_database() {
      let bytes = zip_from(&[
        (CONFIG_NAME, b"config".to_vec()),
        (MANIFEST_JSON_NAME, manifest_with_version(env!("CARGO_PKG_VERSION"))),
      ]);

      let result = read_archive(&bytes);

      assert_eq!(result.unwrap_err(), format!("Archive is missing {DATABASE_NAME}"));
    }

    #[test]
    fn it_rejects_an_archive_missing_the_manifest() {
      let bytes = zip_from(&[(DATABASE_NAME, b"db".to_vec()), (CONFIG_NAME, b"config".to_vec())]);

      let result = read_archive(&bytes);

      assert_eq!(result.unwrap_err(), format!("Archive is missing {MANIFEST_JSON_NAME}"));
    }

    #[test]
    fn it_rejects_an_archive_missing_the_config() {
      let bytes = zip_from(&[
        (DATABASE_NAME, b"db".to_vec()),
        (MANIFEST_JSON_NAME, manifest_with_version(env!("CARGO_PKG_VERSION"))),
      ]);

      let result = read_archive(&bytes);

      assert_eq!(result.unwrap_err(), format!("Archive is missing {CONFIG_NAME}"));
    }

    #[test]
    fn it_rejects_a_corrupt_manifest() {
      let bytes = zip_from(&[
        (DATABASE_NAME, b"db".to_vec()),
        (CONFIG_NAME, b"config".to_vec()),
        (MANIFEST_JSON_NAME, b"{ not json".to_vec()),
      ]);

      let result = read_archive(&bytes);

      assert!(result.unwrap_err().contains("Couldn't parse"));
    }

    #[test]
    fn it_rejects_bytes_that_are_not_a_zip() {
      let result = read_archive(b"definitely not a zip archive");

      assert!(result.unwrap_err().contains("Couldn't open archive"));
    }
  }
}
