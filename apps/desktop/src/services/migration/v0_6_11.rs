use std::path::Path;

use cargo_packager_updater::semver::Version;
use toml_edit::DocumentMut;

use super::{Error, Migrator, Result, local_database_path};
use crate::{
  config,
  store::{self, Database, repo::industry},
};

fn config_error(error: impl std::fmt::Display) -> Error {
  Error::Config(error.to_string())
}

#[allow(non_camel_case_types)]
pub(super) struct V0_6_11;

impl Migrator for V0_6_11 {
  fn version(&self) -> Version {
    Version::new(0, 6, 11)
  }

  async fn after_db_migration(&self) -> Result<()> {
    let Ok(config_path) = config::config_path() else {
      return Ok(());
    };
    let db = store::open_with(&local_database_path()?, async |_writer: &sqlx::SqlitePool| {
      Ok::<(), store::Error>(())
    })
    .await?;
    move_default_facilities(&db, &config_path).await
  }
}

async fn move_default_facilities(db: &Database, path: &Path) -> Result<()> {
  let Some(content) = read_config(path)? else {
    return Ok(());
  };
  let mut document: DocumentMut = content.parse().map_err(config_error)?;
  let manufacturing = take_facility(&mut document, "manufacturing");
  let reactions = take_facility(&mut document, "reactions");
  if manufacturing.is_none() && reactions.is_none() {
    return Ok(());
  }
  industry::import_default_facilities(db, manufacturing, reactions)
    .await
    .map_err(config_error)?;
  prune_empty_industry(&mut document);
  std::fs::write(path, document.to_string()).map_err(config_error)?;
  Ok(())
}

fn read_config(path: &Path) -> Result<Option<String>> {
  match std::fs::read_to_string(path) {
    Ok(content) => Ok(Some(content)),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(error) => Err(config_error(error)),
  }
}

fn take_facility(document: &mut DocumentMut, key: &str) -> Option<i64> {
  let industry = document.get_mut("industry")?.as_table_mut()?;
  let value = industry.get(key).and_then(toml_edit::Item::as_integer);
  industry.remove(key);
  value
}

fn prune_empty_industry(document: &mut DocumentMut) {
  let empty = document
    .get("industry")
    .and_then(toml_edit::Item::as_table)
    .is_some_and(toml_edit::Table::is_empty);
  if empty {
    document.remove("industry");
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;
  use tempfile::tempdir;

  use super::*;
  use crate::store::repo::industry::{MANUFACTURING_ACTIVITY_ID, REACTION_ACTIVITY_ID, default_facility};

  fn write_config(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, body).unwrap();
    (dir, path)
  }

  #[tokio::test]
  async fn it_moves_config_facilities_into_the_database() {
    let db = crate::store::open_test().await.unwrap();
    let (_dir, path) = write_config("[industry]\nmanufacturing = 60003760\nreactions = 1021000000009\n");

    move_default_facilities(&db, &path).await.unwrap();

    assert_eq!(
      default_facility(&db, MANUFACTURING_ACTIVITY_ID).await.unwrap(),
      Some(60003760)
    );
    assert_eq!(
      default_facility(&db, REACTION_ACTIVITY_ID).await.unwrap(),
      Some(1021000000009)
    );
  }

  #[tokio::test]
  async fn it_strips_the_fields_while_preserving_unrelated_comments() {
    let db = crate::store::open_test().await.unwrap();
    let body = "# top of file\n[industry]\n# keep me\nkeep = 7\nmanufacturing = 60003760\n";
    let (_dir, path) = write_config(body);

    move_default_facilities(&db, &path).await.unwrap();

    let result = std::fs::read_to_string(&path).unwrap();
    assert!(!result.contains("manufacturing"), "the field is gone: {result}");
    assert!(
      result.contains("# top of file"),
      "the leading comment survives: {result}"
    );
    assert!(result.contains("# keep me"), "the sibling comment survives: {result}");
    assert!(result.contains("keep = 7"), "the sibling key survives: {result}");
  }

  #[tokio::test]
  async fn it_drops_an_emptied_industry_table() {
    let db = crate::store::open_test().await.unwrap();
    let (_dir, path) = write_config("reprocessing_yield = 0.5\n[industry]\nreactions = 1021000000009\n");

    move_default_facilities(&db, &path).await.unwrap();

    let result = std::fs::read_to_string(&path).unwrap();
    assert!(!result.contains("[industry]"), "the emptied table is removed: {result}");
    assert!(
      result.contains("reprocessing_yield = 0.5"),
      "the sibling key survives: {result}"
    );
  }

  #[tokio::test]
  async fn it_is_a_no_op_when_the_fields_are_already_gone() {
    let db = crate::store::open_test().await.unwrap();
    let body = "# untouched\nreprocessing_yield = 0.5\n";
    let (_dir, path) = write_config(body);

    move_default_facilities(&db, &path).await.unwrap();

    assert_eq!(
      std::fs::read_to_string(&path).unwrap(),
      body,
      "the file is left byte-for-byte"
    );
    assert_eq!(default_facility(&db, MANUFACTURING_ACTIVITY_ID).await.unwrap(), None);
  }

  #[tokio::test]
  async fn it_is_a_no_op_when_the_config_file_is_absent() {
    let db = crate::store::open_test().await.unwrap();
    let dir = tempdir().unwrap();

    move_default_facilities(&db, &dir.path().join("missing.toml"))
      .await
      .unwrap();

    assert_eq!(default_facility(&db, MANUFACTURING_ACTIVITY_ID).await.unwrap(), None);
  }

  #[tokio::test]
  async fn it_preserves_imported_data_on_a_second_run() {
    let db = crate::store::open_test().await.unwrap();
    let (_dir, path) = write_config("[industry]\nmanufacturing = 60003760\n");

    move_default_facilities(&db, &path).await.unwrap();
    move_default_facilities(&db, &path).await.unwrap();

    assert_eq!(
      default_facility(&db, MANUFACTURING_ACTIVITY_ID).await.unwrap(),
      Some(60003760)
    );
  }
}
