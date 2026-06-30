use cargo_packager_updater::semver::Version;
use sha2::{Digest, Sha384};
use sqlx::{SqlitePool, migrate::Migrator as SqlxMigrator};

use super::{Migrator, Result};

#[allow(non_camel_case_types)]
pub(super) struct V0_6_7;

impl Migrator for V0_6_7 {
  fn version(&self) -> Version {
    Version::new(0, 6, 7)
  }

  async fn before_db_migration(&self, pool: &SqlitePool) -> Result<()> {
    let migrator = sqlx::migrate!();
    let healed = repair_crlf_checksums(pool, &migrator).await?;
    if healed > 0 {
      tracing::info!(target: "pod::lifecycle", healed = healed as u64, "repaired CRLF migration checksums");
    }
    Ok(())
  }
}

fn crlf_checksum(migration: &sqlx::migrate::Migration) -> Vec<u8> {
  let crlf = migration.sql.as_str().replace('\n', "\r\n");
  Sha384::digest(crlf.as_bytes()).to_vec()
}

async fn repair_crlf_checksums(
  writer: &SqlitePool,
  migrator: &SqlxMigrator,
) -> std::result::Result<usize, sqlx::Error> {
  let table_exists: i64 =
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')")
      .fetch_one(writer)
      .await?;
  if table_exists == 0 {
    return Ok(0);
  }

  let applied: std::collections::HashMap<i64, Vec<u8>> =
    sqlx::query_as::<_, (i64, Vec<u8>)>("SELECT version, checksum FROM _sqlx_migrations")
      .fetch_all(writer)
      .await?
      .into_iter()
      .collect();

  let mut healed: Vec<(i64, Vec<u8>)> = Vec::new();
  for migration in migrator.iter() {
    let Some(stored) = applied.get(&migration.version) else {
      continue;
    };
    let embedded_lf = migration.checksum.as_ref();
    if stored.as_slice() == embedded_lf {
      continue;
    }
    if stored.as_slice() == crlf_checksum(migration).as_slice() {
      healed.push((migration.version, embedded_lf.to_vec()));
    }
    // A checksum that matches neither variant is a genuine modification; leave it so sqlx rejects it.
  }

  if healed.is_empty() {
    return Ok(0);
  }

  let mut tx = writer.begin().await?;
  for (version, lf_checksum) in &healed {
    sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
      .bind(lf_checksum)
      .bind(version)
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;

  Ok(healed.len())
}

#[cfg(test)]
mod tests {
  use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
  use tempfile::tempdir;

  use super::*;

  async fn migrated_pool() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let options = SqliteConnectOptions::new()
      .filename(dir.path().join("test.db"))
      .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect_with(options)
      .await
      .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    (pool, dir)
  }

  async fn set_stored_checksum(pool: &SqlitePool, version: i64, checksum: &[u8]) {
    sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
      .bind(checksum)
      .bind(version)
      .execute(pool)
      .await
      .unwrap();
  }

  async fn stored_checksum(pool: &SqlitePool, version: i64) -> Vec<u8> {
    sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version = ?")
      .bind(version)
      .fetch_one(pool)
      .await
      .unwrap()
  }

  #[tokio::test]
  async fn it_heals_a_crlf_checksum_so_migrate_succeeds_again() {
    let (pool, _dir) = migrated_pool().await;
    let migrator = sqlx::migrate!();
    let first = migrator.iter().next().unwrap();

    set_stored_checksum(&pool, first.version, &crlf_checksum(first)).await;

    let err = sqlx::migrate!().run(&pool).await.unwrap_err();
    assert!(
      matches!(err, sqlx::migrate::MigrateError::VersionMismatch(v) if v == first.version),
      "a CRLF-era checksum must fail validation before the repair runs (got {err:?})"
    );

    let healed = repair_crlf_checksums(&pool, &migrator).await.unwrap();
    assert_eq!(healed, 1, "exactly the one CRLF-twin checksum is healed");
    assert_eq!(
      stored_checksum(&pool, first.version).await,
      first.checksum.as_ref(),
      "the stored checksum is rewritten to the embedded LF value"
    );

    sqlx::migrate!().run(&pool).await.unwrap();
  }

  #[tokio::test]
  async fn it_leaves_a_genuinely_modified_checksum_untouched() {
    let (pool, _dir) = migrated_pool().await;
    let migrator = sqlx::migrate!();
    let first = migrator.iter().next().unwrap();

    let tampered = vec![0xAB_u8; 48];
    set_stored_checksum(&pool, first.version, &tampered).await;

    let healed = repair_crlf_checksums(&pool, &migrator).await.unwrap();
    assert_eq!(healed, 0, "a genuine modification is never auto-healed");
    assert_eq!(
      stored_checksum(&pool, first.version).await,
      tampered,
      "the tampered checksum is left exactly as-is"
    );
    let err = sqlx::migrate!().run(&pool).await.unwrap_err();
    assert!(matches!(err, sqlx::migrate::MigrateError::VersionMismatch(v) if v == first.version));
  }

  #[tokio::test]
  async fn it_is_a_no_op_on_a_healthy_database_and_idempotent() {
    let (pool, _dir) = migrated_pool().await;
    let migrator = sqlx::migrate!();

    assert_eq!(repair_crlf_checksums(&pool, &migrator).await.unwrap(), 0);

    let first = migrator.iter().next().unwrap();
    set_stored_checksum(&pool, first.version, &crlf_checksum(first)).await;
    assert_eq!(repair_crlf_checksums(&pool, &migrator).await.unwrap(), 1);
    assert_eq!(repair_crlf_checksums(&pool, &migrator).await.unwrap(), 0);
  }

  #[tokio::test]
  async fn it_is_a_no_op_when_the_migrations_table_is_absent() {
    let dir = tempdir().unwrap();
    let options = SqliteConnectOptions::new()
      .filename(dir.path().join("fresh.db"))
      .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect_with(options)
      .await
      .unwrap();

    assert_eq!(repair_crlf_checksums(&pool, &sqlx::migrate!()).await.unwrap(), 0);
  }
}
