use chrono::Utc;

use crate::store::{
  Database, Error,
  model::{OwnerType, SyncLedger},
};

// Public store API exercised by unit tests; not yet wired into a production call site.
#[cfg_attr(not(test), expect(dead_code))]
pub async fn all(db: &Database) -> Result<Vec<SyncLedger>, Error> {
  let rows = sqlx::query_as::<_, SyncLedger>(
    "SELECT kind, last_attempt_at, last_reason, last_success_at, next_eligible_at, outcome, rows_touched, \
    subject_id, subject_type FROM sync_ledger",
  )
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Deletes every ledger row whose kind is in `kinds`, across all subjects, so the engine re-dispatches
// those jobs as never-attempted on the next discovery pass. Used to force a re-fetch on a detected
// language switch (ADR-0041 section 4), reusing ADR-0040's `DELETE FROM sync_ledger WHERE kind IN (...)`
// precedent. Idempotent: expiring an already-expired (absent) row deletes nothing.
pub async fn expire_kinds(db: &Database, kinds: &[&str]) -> Result<u64, Error> {
  if kinds.is_empty() {
    return Ok(0);
  }

  let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new("DELETE FROM sync_ledger WHERE kind IN (");
  let mut separated = builder.separated(", ");
  for kind in kinds {
    separated.push_bind(*kind);
  }
  builder.push(")");

  let result = builder.build().execute(db.writer()).await?;
  Ok(result.rows_affected())
}

pub async fn for_subject(db: &Database, subject_type: OwnerType, subject_id: i64) -> Result<Vec<SyncLedger>, Error> {
  let rows = sqlx::query_as::<_, SyncLedger>(
    "SELECT kind, last_attempt_at, last_reason, last_success_at, next_eligible_at, outcome, rows_touched, \
    subject_id, subject_type FROM sync_ledger WHERE subject_type = ? AND subject_id = ?",
  )
  .bind(subject_type)
  .bind(subject_id)
  .fetch_all(&db.0)
  .await?;
  Ok(rows)
}

// Public store API exercised by unit tests; not yet wired into a production call site.
#[cfg_attr(not(test), expect(dead_code))]
pub async fn get(
  db: &Database,
  subject_type: OwnerType,
  subject_id: i64,
  kind: &str,
) -> Result<Option<SyncLedger>, Error> {
  let row = sqlx::query_as::<_, SyncLedger>(
    "SELECT kind, last_attempt_at, last_reason, last_success_at, next_eligible_at, outcome, rows_touched, \
    subject_id, subject_type FROM sync_ledger WHERE subject_type = ? AND subject_id = ? AND kind = ?",
  )
  .bind(subject_type)
  .bind(subject_id)
  .bind(kind)
  .fetch_optional(&db.0)
  .await?;
  Ok(row)
}

// Arguments map directly to the sync_ledger row columns; bundling them into a struct would only move the fields.
#[allow(clippy::too_many_arguments)]
pub async fn upsert(
  db: &Database,
  subject_type: OwnerType,
  subject_id: i64,
  kind: &str,
  outcome: &str,
  rows_touched: i64,
  reason: Option<&str>,
  success_at: Option<&str>,
  next_eligible_at: Option<&str>,
) -> Result<(), Error> {
  let now = Utc::now().to_rfc3339();
  sqlx::query(
    "INSERT INTO sync_ledger \
      (subject_type, subject_id, kind, outcome, rows_touched, last_reason, last_attempt_at, last_success_at, \
      next_eligible_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT(subject_type, subject_id, kind) DO UPDATE SET \
      outcome          = excluded.outcome, \
      rows_touched     = excluded.rows_touched, \
      last_reason      = excluded.last_reason, \
      last_attempt_at  = excluded.last_attempt_at, \
      last_success_at  = COALESCE(excluded.last_success_at, sync_ledger.last_success_at), \
      next_eligible_at = excluded.next_eligible_at",
  )
  .bind(subject_type)
  .bind(subject_id)
  .bind(kind)
  .bind(outcome)
  .bind(rows_touched)
  .bind(reason)
  .bind(&now)
  .bind(success_at)
  .bind(next_eligible_at)
  .execute(db.writer())
  .await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  const CHARACTER: i64 = 95_465_499;

  mod expire_kinds {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn seed_row(db: &Database, subject_type: OwnerType, subject_id: i64, kind: &str) {
      upsert(db, subject_type, subject_id, kind, "synced", 1, None, None, None)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_deletes_only_the_requested_kinds_across_all_subjects() {
      let db = store::open_test().await.unwrap();
      seed_row(&db, OwnerType::Character, CHARACTER, "AssetSync").await;
      seed_row(&db, OwnerType::Character, 90_000_002, "AssetSync").await;
      seed_row(&db, OwnerType::Corporation, 98_000_001, "CorporationStructures").await;
      seed_row(&db, OwnerType::Character, CHARACTER, "MarketPrices").await;

      let deleted = expire_kinds(&db, &["AssetSync", "CorporationStructures"])
        .await
        .unwrap();

      assert_eq!(deleted, 3);
      let remaining = all(&db).await.unwrap();
      assert_eq!(remaining.len(), 1);
      assert_eq!(remaining[0].kind(), "MarketPrices");
    }

    #[tokio::test]
    async fn it_is_a_no_op_for_an_empty_kind_set() {
      let db = store::open_test().await.unwrap();
      seed_row(&db, OwnerType::Character, CHARACTER, "AssetSync").await;

      let deleted = expire_kinds(&db, &[]).await.unwrap();

      assert_eq!(deleted, 0);
      assert_eq!(all(&db).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn it_deletes_nothing_when_no_row_matches() {
      let db = store::open_test().await.unwrap();
      seed_row(&db, OwnerType::Character, CHARACTER, "MarketPrices").await;

      let deleted = expire_kinds(&db, &["AssetSync"]).await.unwrap();

      assert_eq!(deleted, 0);
      assert_eq!(all(&db).await.unwrap().len(), 1);
    }
  }

  mod for_subject {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_only_the_rows_for_the_requested_subject() {
      let db = store::open_test().await.unwrap();
      upsert(
        &db,
        OwnerType::Character,
        CHARACTER,
        "AssetSync",
        "synced",
        1,
        None,
        None,
        None,
      )
      .await
      .unwrap();
      upsert(
        &db,
        OwnerType::Character,
        CHARACTER,
        "CharacterWallet",
        "synced",
        2,
        None,
        None,
        None,
      )
      .await
      .unwrap();
      upsert(
        &db,
        OwnerType::Character,
        90_000_002,
        "AssetSync",
        "synced",
        3,
        None,
        None,
        None,
      )
      .await
      .unwrap();

      let rows = for_subject(&db, OwnerType::Character, CHARACTER).await.unwrap();

      assert_eq!(rows.len(), 2);
      assert!(rows.iter().all(|row| row.subject_id() == CHARACTER));
    }
  }

  mod get {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_returns_none_when_no_row_exists() {
      let db = store::open_test().await.unwrap();

      let row = get(&db, OwnerType::Character, CHARACTER, "AssetSync").await.unwrap();

      assert_eq!(row, None);
    }
  }

  mod upsert {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_inserts_a_new_ledger_row() {
      let db = store::open_test().await.unwrap();

      upsert(
        &db,
        OwnerType::Character,
        CHARACTER,
        "AssetSync",
        "synced",
        12,
        None,
        Some("2026-01-01T00:00:00+00:00"),
        Some("2026-01-01T01:00:00+00:00"),
      )
      .await
      .unwrap();

      let row = get(&db, OwnerType::Character, CHARACTER, "AssetSync")
        .await
        .unwrap()
        .unwrap();
      assert_eq!(row.subject_type(), OwnerType::Character);
      assert_eq!(row.subject_id(), CHARACTER);
      assert_eq!(row.kind(), "AssetSync");
      assert_eq!(row.outcome(), "synced");
      assert_eq!(row.rows_touched(), 12);
      assert_eq!(row.last_reason(), &None);
      assert_eq!(row.last_success_at().as_deref(), Some("2026-01-01T00:00:00+00:00"));
      assert_eq!(row.next_eligible_at().as_deref(), Some("2026-01-01T01:00:00+00:00"));
    }

    #[tokio::test]
    async fn it_keys_rows_independently_per_kind_and_subject() {
      let db = store::open_test().await.unwrap();

      upsert(
        &db,
        OwnerType::Character,
        CHARACTER,
        "AssetSync",
        "synced",
        1,
        None,
        None,
        None,
      )
      .await
      .unwrap();
      upsert(
        &db,
        OwnerType::Character,
        CHARACTER,
        "CharacterWallet",
        "empty",
        0,
        None,
        None,
        None,
      )
      .await
      .unwrap();
      upsert(
        &db,
        OwnerType::Corporation,
        98_000_001,
        "AssetSync",
        "synced",
        9,
        None,
        None,
        None,
      )
      .await
      .unwrap();

      assert_eq!(all(&db).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn it_overwrites_the_existing_row_for_the_same_subject_and_kind() {
      let db = store::open_test().await.unwrap();
      upsert(
        &db,
        OwnerType::Character,
        CHARACTER,
        "AssetSync",
        "synced",
        5,
        None,
        None,
        None,
      )
      .await
      .unwrap();

      upsert(
        &db,
        OwnerType::Character,
        CHARACTER,
        "AssetSync",
        "blocked",
        0,
        Some("missing scope"),
        None,
        None,
      )
      .await
      .unwrap();

      let rows = all(&db).await.unwrap();
      assert_eq!(rows.len(), 1, "a second attempt updates the row in place, not appends");
      assert_eq!(rows[0].outcome(), "blocked");
      assert_eq!(rows[0].rows_touched(), 0);
      assert_eq!(rows[0].last_reason().as_deref(), Some("missing scope"));
    }

    #[tokio::test]
    async fn it_preserves_a_prior_success_when_a_later_attempt_does_not_succeed() {
      let db = store::open_test().await.unwrap();
      upsert(
        &db,
        OwnerType::Character,
        CHARACTER,
        "AssetSync",
        "synced",
        5,
        None,
        Some("2026-01-01T00:00:00+00:00"),
        None,
      )
      .await
      .unwrap();

      upsert(
        &db,
        OwnerType::Character,
        CHARACTER,
        "AssetSync",
        "failed",
        0,
        Some("timeout"),
        None,
        None,
      )
      .await
      .unwrap();

      let row = get(&db, OwnerType::Character, CHARACTER, "AssetSync")
        .await
        .unwrap()
        .unwrap();
      assert_eq!(row.outcome(), "failed");
      assert_eq!(
        row.last_success_at().as_deref(),
        Some("2026-01-01T00:00:00+00:00"),
        "a failed attempt must not erase proven freshness"
      );
    }

    #[tokio::test]
    async fn it_rejects_an_outcome_outside_the_allowed_set() {
      let db = store::open_test().await.unwrap();

      let result = upsert(
        &db,
        OwnerType::Character,
        CHARACTER,
        "AssetSync",
        "bogus",
        0,
        None,
        None,
        None,
      )
      .await;

      assert!(
        result.is_err(),
        "the CHECK constraint must reject an unknown outcome token"
      );
    }
  }
}
