use cargo_packager_updater::semver::Version;
use sqlx::SqlitePool;

use crate::{config::Settings, store::Database};

mod v0_6_7;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("migration database error: {0}")]
  Sqlx(#[from] sqlx::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[allow(async_fn_in_trait)]
pub trait Migrator {
  fn version(&self) -> Version;

  async fn before_db_migration(&self, _pool: &SqlitePool) -> Result<()> {
    Ok(())
  }

  async fn after_db_migration(&self, _db: &Database, _config: &mut Settings) -> Result<()> {
    Ok(())
  }
}

#[allow(non_camel_case_types)]
enum Registered {
  V0_6_7(v0_6_7::V0_6_7),
}

impl Registered {
  fn version(&self) -> Version {
    match self {
      Self::V0_6_7(migrator) => migrator.version(),
    }
  }

  async fn before_db_migration(&self, pool: &SqlitePool) -> Result<()> {
    match self {
      Self::V0_6_7(migrator) => migrator.before_db_migration(pool).await,
    }
  }

  async fn after_db_migration(&self, db: &Database, config: &mut Settings) -> Result<()> {
    match self {
      Self::V0_6_7(migrator) => migrator.after_db_migration(db, config).await,
    }
  }
}

fn registered() -> Vec<Registered> {
  vec![Registered::V0_6_7(v0_6_7::V0_6_7)]
}

fn current() -> Version {
  Version::parse(env!("CARGO_PKG_VERSION")).expect("CARGO_PKG_VERSION is valid semver")
}

fn parse_pod_token(marker: &str) -> Option<Version> {
  marker
    .split('+')
    .find_map(|segment| segment.strip_prefix("pod-"))
    .and_then(|token| Version::parse(token).ok())
}

fn from_version(marker: Option<&str>, db_present: bool) -> Option<Version> {
  if let Some(marker) = marker
    && let Some(version) = parse_pod_token(marker)
  {
    return Some(version);
  }
  if db_present {
    return Some(Version::new(0, 6, 0));
  }
  None
}

pub struct Registry {
  applicable: Vec<Registered>,
}

impl Registry {
  pub fn resolve(marker: Option<&str>, db_present: bool) -> Self {
    Self::resolve_with(marker, db_present, &current())
  }

  fn resolve_with(marker: Option<&str>, db_present: bool, current: &Version) -> Self {
    let Some(from) = from_version(marker, db_present) else {
      return Self {
        applicable: Vec::new(),
      };
    };
    let mut applicable: Vec<Registered> = registered()
      .into_iter()
      .filter(|migrator| migrator.version() > from && migrator.version() <= *current)
      .collect();
    applicable.sort_by_key(Registered::version);
    Self {
      applicable,
    }
  }

  pub fn is_empty(&self) -> bool {
    self.applicable.is_empty()
  }

  pub async fn before_db_migration(&self, writer: &SqlitePool) -> Result<()> {
    for migrator in &self.applicable {
      migrator.before_db_migration(writer).await?;
    }
    Ok(())
  }

  pub async fn after_db_migration(&self, db: &Database, config: &mut Settings) -> Result<()> {
    for migrator in &self.applicable {
      migrator.after_db_migration(db, config).await?;
    }
    Ok(())
  }

  #[cfg(test)]
  fn applicable_versions(&self) -> Vec<Version> {
    self.applicable.iter().map(Registered::version).collect()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod resolve {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_selects_the_migrators_in_range_after_an_upgrade() {
      let registry = Registry::resolve_with(Some("12345+pod-0.6.6+seed-2+lang-en"), true, &Version::new(0, 6, 7));

      assert_eq!(
        registry.applicable_versions(),
        vec![Version::new(0, 6, 7)],
        "a 0.6.6 install upgrading to 0.6.7 runs the 0.6.7 migrator"
      );
    }

    #[test]
    fn it_skips_every_migrator_on_a_fresh_install() {
      let registry = Registry::resolve_with(None, false, &Version::new(0, 6, 7));

      assert!(
        registry.is_empty(),
        "a fresh install with no marker and no database is treated as current"
      );
    }

    #[test]
    fn it_floors_a_markerless_existing_database_to_0_6_0() {
      let registry = Registry::resolve_with(None, true, &Version::new(0, 6, 7));

      assert_eq!(
        registry.applicable_versions(),
        vec![Version::new(0, 6, 7)],
        "a database without a marker is floored to 0.6.0 so the 0.6.7 migrator runs"
      );
    }

    #[test]
    fn it_is_a_no_op_re_run_once_the_marker_records_the_current_version() {
      let registry = Registry::resolve_with(Some("12345+pod-0.6.7+seed-2+lang-en"), true, &Version::new(0, 6, 7));

      assert!(
        registry.is_empty(),
        "re-running against a marker already at the current version selects nothing"
      );
    }

    #[test]
    fn it_excludes_migrators_newer_than_the_running_build() {
      let registry = Registry::resolve_with(Some("12345+pod-0.6.5+seed-2+lang-en"), true, &Version::new(0, 6, 6));

      assert!(
        registry.is_empty(),
        "the 0.6.7 migrator never runs on a 0.6.6 build even when the from-version is older"
      );
    }
  }

  mod from_version {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_the_pod_token_from_a_full_marker() {
      assert_eq!(
        super::super::from_version(Some("20240101.1+pod-0.6.6+seed-2+lang-en"), true),
        Some(Version::new(0, 6, 6))
      );
    }

    #[test]
    fn it_floors_an_unparseable_marker_with_a_database_to_0_6_0() {
      assert_eq!(
        super::super::from_version(Some("garbage-without-a-pod-token"), true),
        Some(Version::new(0, 6, 0))
      );
    }

    #[test]
    fn it_returns_none_for_a_fresh_install() {
      assert_eq!(super::super::from_version(None, false), None);
    }
  }
}
