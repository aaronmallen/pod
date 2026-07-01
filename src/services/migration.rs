use std::path::PathBuf;

use cargo_packager_updater::semver::Version;

use crate::{config, store};

mod v0_6_11;
mod v0_6_8;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("migration config error: {0}")]
  Config(String),
  #[error("migration database error: {0}")]
  Sqlx(#[from] sqlx::Error),
  #[error("migration store error: {0}")]
  Store(#[from] store::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[allow(async_fn_in_trait)]
pub trait Migrator {
  fn version(&self) -> Version;

  async fn before_db_migration(&self) -> Result<()> {
    Ok(())
  }

  async fn after_db_migration(&self) -> Result<()> {
    Ok(())
  }
}

#[allow(non_camel_case_types)]
enum Registered {
  V0_6_8(v0_6_8::V0_6_8),
  V0_6_11(v0_6_11::V0_6_11),
}

impl Registered {
  fn version(&self) -> Version {
    match self {
      Self::V0_6_8(migrator) => migrator.version(),
      Self::V0_6_11(migrator) => migrator.version(),
    }
  }

  async fn before_db_migration(&self) -> Result<()> {
    match self {
      Self::V0_6_8(migrator) => migrator.before_db_migration().await,
      Self::V0_6_11(migrator) => migrator.before_db_migration().await,
    }
  }

  async fn after_db_migration(&self) -> Result<()> {
    match self {
      Self::V0_6_8(migrator) => migrator.after_db_migration().await,
      Self::V0_6_11(migrator) => migrator.after_db_migration().await,
    }
  }
}

fn registered() -> Vec<Registered> {
  vec![
    Registered::V0_6_8(v0_6_8::V0_6_8),
    Registered::V0_6_11(v0_6_11::V0_6_11),
  ]
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

pub(super) fn local_database_path() -> Result<PathBuf> {
  let settings = config::load().map_err(|error| Error::Config(error.to_string()))?;
  Ok(store::bootstrap::local_path(settings.storage()))
}

fn from_version(marker: Option<&str>, db_present: bool) -> Option<Version> {
  if let Some(marker) = marker
    && let Some(version) = parse_pod_token(marker)
  {
    return Some(version);
  }
  // Pre-0.6.7 databases predate the pod-version marker; treat as 0.6.0 so all subsequent migrators run.
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

  // Only the resolve tests still assert on emptiness now that boot no longer re-saves config purely
  // because a migrator ran; keep it callable without tripping dead-code in the non-test build.
  #[cfg_attr(not(test), expect(dead_code))]
  pub fn is_empty(&self) -> bool {
    self.applicable.is_empty()
  }

  pub async fn before_db_migration(&self) -> Result<()> {
    for migrator in &self.applicable {
      migrator.before_db_migration().await?;
    }
    Ok(())
  }

  pub async fn after_db_migration(&self) -> Result<()> {
    for migrator in &self.applicable {
      migrator.after_db_migration().await?;
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
      let registry = Registry::resolve_with(Some("12345+pod-0.6.6+seed-2+lang-en"), true, &Version::new(0, 6, 8));

      assert_eq!(
        registry.applicable_versions(),
        vec![Version::new(0, 6, 8)],
        "a 0.6.6 install upgrading to 0.6.8 runs the 0.6.8 migrator"
      );
    }

    #[test]
    fn it_still_heals_a_0_6_7_install_that_never_received_the_store_open_heal() {
      let registry = Registry::resolve_with(Some("12345+pod-0.6.7+seed-2+lang-en"), true, &Version::new(0, 6, 8));

      assert_eq!(
        registry.applicable_versions(),
        vec![Version::new(0, 6, 8)],
        "the CRLF heal first shipped in 0.6.8, so a 0.6.7 install still needs the 0.6.8 migrator on upgrade"
      );
    }

    #[test]
    fn it_skips_every_migrator_on_a_fresh_install() {
      let registry = Registry::resolve_with(None, false, &Version::new(0, 6, 8));

      assert!(
        registry.is_empty(),
        "a fresh install with no marker and no database is treated as current"
      );
    }

    #[test]
    fn it_floors_a_markerless_existing_database_to_0_6_0() {
      let registry = Registry::resolve_with(None, true, &Version::new(0, 6, 8));

      assert_eq!(
        registry.applicable_versions(),
        vec![Version::new(0, 6, 8)],
        "a database without a marker is floored to 0.6.0 so the 0.6.8 migrator runs"
      );
    }

    #[test]
    fn it_is_a_no_op_re_run_once_the_marker_records_the_current_version() {
      let registry = Registry::resolve_with(Some("12345+pod-0.6.8+seed-2+lang-en"), true, &Version::new(0, 6, 8));

      assert!(
        registry.is_empty(),
        "re-running against a marker already at the current version selects nothing"
      );
    }

    #[test]
    fn it_excludes_migrators_newer_than_the_running_build() {
      let registry = Registry::resolve_with(Some("12345+pod-0.6.6+seed-2+lang-en"), true, &Version::new(0, 6, 7));

      assert!(
        registry.is_empty(),
        "the 0.6.8 migrator never runs on a 0.6.7 build even when the from-version is older"
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
