use std::path::Path;

use super::job::JobKind;
use crate::{
  features::splash::seed,
  i18n::Language,
  store::{Database, Error, repo::sync_ledger},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refresh {
  NoSwitch,
  Switched { expired: u64 },
}

fn language_dependent_kind_tokens() -> Vec<String> {
  JobKind::ALL
    .iter()
    .copied()
    .filter(|kind| kind.is_language_dependent())
    .map(|kind| format!("{kind:?}"))
    .collect()
}

// Forces a re-fetch of the language-dependent jobs when the synced-language marker disagrees with the
// configured language, then advances the marker so an uninterrupted switch does not re-trigger. On a
// genuine first run (no marker) it records the configured language without expiring anything. This is
// the boot-time hook ADR-0041 sections 3 and 4 describe; it runs before the engine's first discovery
// pass so the expired jobs present as never-attempted and fire on the first scheduling pass.
pub async fn refresh_for_language_switch(
  db: &Database,
  configured: Language,
  marker_path: &Path,
) -> Result<Refresh, Error> {
  let marker = seed::read_synced_language(marker_path);

  if !seed::language_switched(marker, configured) {
    if marker.is_none() {
      seed::write_synced_language(marker_path, configured);
    }
    return Ok(Refresh::NoSwitch);
  }

  let tokens = language_dependent_kind_tokens();
  let kinds: Vec<&str> = tokens.iter().map(String::as_str).collect();
  let expired = sync_ledger::expire_kinds(db, &kinds).await?;

  seed::write_synced_language(marker_path, configured);

  Ok(Refresh::Switched {
    expired,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  mod language_dependent_kind_tokens {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_lists_exactly_the_language_dependent_kinds_as_debug_strings() {
      let tokens = language_dependent_kind_tokens();

      let expected: Vec<String> = JobKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_language_dependent())
        .map(|kind| format!("{kind:?}"))
        .collect();

      assert_eq!(tokens, expected);
      assert!(tokens.contains(&"AssetSync".to_owned()));
      assert!(!tokens.contains(&"MarketPrices".to_owned()));
    }
  }

  mod refresh_for_language_switch {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::store::{self, model::OwnerType, repo::sync_ledger};

    const CHARACTER: i64 = 95_465_499;

    const FUTURE: &str = "2999-01-01T00:00:00+00:00";

    async fn seed_fresh_row(db: &Database, kind: &str) {
      sync_ledger::upsert(
        db,
        OwnerType::Character,
        CHARACTER,
        kind,
        "synced",
        1,
        None,
        Some("2026-01-01T00:00:00+00:00"),
        Some(FUTURE),
      )
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_expires_language_dependent_rows_and_advances_the_marker_on_a_switch() {
      let db = store::open_test().await.unwrap();
      seed_fresh_row(&db, "AssetSync").await;
      seed_fresh_row(&db, "CorporationStructures").await;
      seed_fresh_row(&db, "MarketPrices").await;
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("synced_language");
      seed::write_synced_language(&marker, Language::En);

      let result = refresh_for_language_switch(&db, Language::Fr, &marker).await.unwrap();

      assert_eq!(
        result,
        Refresh::Switched {
          expired: 2
        }
      );

      let rows = sync_ledger::all(&db).await.unwrap();
      assert_eq!(rows.len(), 1, "only the non-language-dependent row survives");
      assert_eq!(rows[0].kind(), "MarketPrices");
      assert_eq!(
        rows[0].next_eligible_at().as_deref(),
        Some(FUTURE),
        "a surviving job keeps its next-run, so it is not forced due-now"
      );

      assert_eq!(seed::read_synced_language(&marker), Some(Language::Fr));
    }

    #[tokio::test]
    async fn it_does_nothing_when_the_marker_already_matches() {
      let db = store::open_test().await.unwrap();
      seed_fresh_row(&db, "AssetSync").await;
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("synced_language");
      seed::write_synced_language(&marker, Language::De);

      let result = refresh_for_language_switch(&db, Language::De, &marker).await.unwrap();

      assert_eq!(result, Refresh::NoSwitch);
      assert_eq!(sync_ledger::all(&db).await.unwrap().len(), 1);
      assert_eq!(seed::read_synced_language(&marker), Some(Language::De));
    }

    #[tokio::test]
    async fn it_records_the_marker_without_expiring_on_a_first_run() {
      let db = store::open_test().await.unwrap();
      seed_fresh_row(&db, "AssetSync").await;
      let tmp = tempfile::tempdir().unwrap();
      let marker = tmp.path().join("synced_language");

      let result = refresh_for_language_switch(&db, Language::Ja, &marker).await.unwrap();

      assert_eq!(result, Refresh::NoSwitch);
      assert_eq!(
        sync_ledger::all(&db).await.unwrap().len(),
        1,
        "a first run already syncs in the chosen language, so it forces no re-fetch"
      );
      assert_eq!(seed::read_synced_language(&marker), Some(Language::Ja));
    }
  }
}
