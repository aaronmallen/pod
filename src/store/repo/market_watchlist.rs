use crate::store::{
  Database, Error,
  model::{MarketWatch, NewWatch},
};

#[allow(dead_code)]
pub async fn list(db: &Database) -> Result<Vec<MarketWatch>, Error> {
  let rows = sqlx::query_as::<_, MarketWatch>(
    "SELECT created_at, direction, id, location_id, location_tier, region_id, target_price, type_id, updated_at \
    FROM market_watchlist ORDER BY sort_order ASC, id DESC",
  )
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn get(db: &Database, id: i64) -> Result<Option<MarketWatch>, Error> {
  let row = sqlx::query_as::<_, MarketWatch>(
    "SELECT created_at, direction, id, location_id, location_tier, region_id, target_price, type_id, updated_at \
    FROM market_watchlist WHERE id = ?",
  )
  .bind(id)
  .fetch_optional(db.reader())
  .await?;
  Ok(row)
}

#[allow(dead_code)]
pub async fn create(db: &Database, input: &NewWatch) -> Result<MarketWatch, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let row = sqlx::query_as::<_, MarketWatch>(
    "INSERT INTO market_watchlist \
      (created_at, direction, location_id, location_tier, region_id, sort_order, target_price, type_id, updated_at) \
    VALUES (?, ?, ?, ?, ?, (SELECT COALESCE(MIN(sort_order), 1) - 1 FROM market_watchlist), ?, ?, ?) \
    RETURNING created_at, direction, id, location_id, location_tier, region_id, target_price, type_id, updated_at",
  )
  .bind(&now)
  .bind(input.direction.as_str())
  .bind(input.location_id)
  .bind(input.location_tier.as_deref())
  .bind(input.region_id)
  .bind(input.target_price)
  .bind(input.type_id)
  .bind(&now)
  .fetch_one(db.writer())
  .await?;
  Ok(row)
}

#[allow(dead_code)]
pub async fn update(db: &Database, id: i64, input: &NewWatch) -> Result<u64, Error> {
  let now = chrono::Utc::now().to_rfc3339();
  let result = sqlx::query(
    "UPDATE market_watchlist \
    SET direction = ?, location_id = ?, location_tier = ?, region_id = ?, target_price = ?, type_id = ?, updated_at = ? \
    WHERE id = ?",
  )
  .bind(input.direction.as_str())
  .bind(input.location_id)
  .bind(input.location_tier.as_deref())
  .bind(input.region_id)
  .bind(input.target_price)
  .bind(input.type_id)
  .bind(&now)
  .bind(id)
  .execute(db.writer())
  .await?;
  Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn delete(db: &Database, id: i64) -> Result<u64, Error> {
  let result = sqlx::query("DELETE FROM market_watchlist WHERE id = ?")
    .bind(id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

pub async fn reorder(db: &Database, ordered_ids: &[i64]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;
  for (position, id) in ordered_ids.iter().enumerate() {
    sqlx::query("UPDATE market_watchlist SET sort_order = ? WHERE id = ?")
      .bind(position as i64)
      .bind(id)
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store::{self, model::WatchDirection};

  fn watch() -> NewWatch {
    NewWatch {
      direction: WatchDirection::Buy,
      location_id: Some(60_003_760),
      location_tier: Some("station".to_owned()),
      region_id: Some(10_000_002),
      target_price: Some(5_000_000.0),
      type_id: 34,
    }
  }

  mod crud {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_and_reads_back_a_watch() {
      let db = store::open_test().await.unwrap();

      let created = create(&db, &watch()).await.unwrap();

      assert!(created.id > 0);
      assert_eq!(created.direction, "buy");
      assert_eq!(created.type_id, 34);
      assert_eq!(created.location_id, Some(60_003_760));
      assert_eq!(created.location_tier, Some("station".to_owned()));
      assert_eq!(created.region_id, Some(10_000_002));
      assert_eq!(created.target_price, Some(5_000_000.0));
      assert_eq!(created.created_at, created.updated_at);

      let fetched = get(&db, created.id).await.unwrap().unwrap();
      assert_eq!(fetched, created);
    }

    #[tokio::test]
    async fn it_updates_the_editable_fields() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, &watch()).await.unwrap();

      let mut edit = watch();
      edit.direction = WatchDirection::Sell;
      edit.target_price = Some(9_000_000.0);
      edit.location_id = None;
      let affected = update(&db, created.id, &edit).await.unwrap();

      assert_eq!(affected, 1);
      let fetched = get(&db, created.id).await.unwrap().unwrap();
      assert_eq!(fetched.direction, "sell");
      assert_eq!(fetched.target_price, Some(9_000_000.0));
      assert_eq!(fetched.location_id, None);
    }

    #[tokio::test]
    async fn it_lists_every_watch_newest_first() {
      let db = store::open_test().await.unwrap();
      let first = create(&db, &watch()).await.unwrap();
      let second = create(&db, &watch()).await.unwrap();
      let third = create(&db, &watch()).await.unwrap();

      let all = list(&db).await.unwrap();

      assert_eq!(all.len(), 3);
      assert_eq!(all[0].id, third.id);
      assert_eq!(all[1].id, second.id);
      assert_eq!(all[2].id, first.id);
    }

    #[tokio::test]
    async fn it_deletes_a_watch() {
      let db = store::open_test().await.unwrap();
      let created = create(&db, &watch()).await.unwrap();

      let affected = delete(&db, created.id).await.unwrap();

      assert_eq!(affected, 1);
      assert_eq!(get(&db, created.id).await.unwrap(), None);
    }
  }

  mod sort_order {
    use pretty_assertions::assert_eq;

    use super::*;

    const MIGRATION: &str = include_str!("../../../migrations/0151_add_market_watchlist_sort_order.sql");

    async fn ids(db: &Database) -> Vec<i64> {
      list(db).await.unwrap().iter().map(|row| row.id).collect()
    }

    #[tokio::test]
    async fn it_inserts_new_watches_at_the_top() {
      let db = store::open_test().await.unwrap();
      let first = create(&db, &watch()).await.unwrap();
      let second = create(&db, &watch()).await.unwrap();
      let third = create(&db, &watch()).await.unwrap();

      assert_eq!(ids(&db).await, vec![third.id, second.id, first.id]);
    }

    #[tokio::test]
    async fn it_persists_a_full_batch_reorder() {
      let db = store::open_test().await.unwrap();
      let first = create(&db, &watch()).await.unwrap();
      let second = create(&db, &watch()).await.unwrap();
      let third = create(&db, &watch()).await.unwrap();

      reorder(&db, &[first.id, third.id, second.id]).await.unwrap();

      assert_eq!(ids(&db).await, vec![first.id, third.id, second.id]);
    }

    #[tokio::test]
    async fn it_keeps_inserting_at_the_top_after_a_reorder() {
      let db = store::open_test().await.unwrap();
      let first = create(&db, &watch()).await.unwrap();
      let second = create(&db, &watch()).await.unwrap();
      reorder(&db, &[first.id, second.id]).await.unwrap();

      let third = create(&db, &watch()).await.unwrap();

      assert_eq!(ids(&db).await, vec![third.id, first.id, second.id]);
    }

    #[tokio::test]
    async fn it_backfills_from_created_at_desc_ordering() {
      let db = store::open_test().await.unwrap();
      let first = create(&db, &watch()).await.unwrap();
      let second = create(&db, &watch()).await.unwrap();
      let third = create(&db, &watch()).await.unwrap();
      let stamps = [
        (first.id, "2026-07-01T00:00:00Z"),
        (second.id, "2026-07-03T00:00:00Z"),
        (third.id, "2026-07-02T00:00:00Z"),
      ];
      for (id, created_at) in stamps {
        sqlx::query("UPDATE market_watchlist SET created_at = ?, sort_order = 0 WHERE id = ?")
          .bind(created_at)
          .bind(id)
          .execute(db.writer())
          .await
          .unwrap();
      }

      let backfill = MIGRATION.split_once(';').unwrap().1.trim().trim_end_matches(';');
      sqlx::query(backfill).execute(db.writer()).await.unwrap();

      assert_eq!(ids(&db).await, vec![second.id, third.id, first.id]);
    }
  }

  mod migrate_global {
    use pretty_assertions::assert_eq;

    use super::*;

    const MIGRATION: &str = include_str!("../../../migrations/0155_drop_market_watchlist_character_id.sql");

    async fn stamp(db: &Database, id: i64, updated_at: &str) {
      sqlx::query("UPDATE market_watchlist SET updated_at = ? WHERE id = ?")
        .bind(updated_at)
        .bind(id)
        .execute(db.writer())
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn it_dedupes_identical_scope_rows_keeping_the_most_recently_updated() {
      let db = store::open_test().await.unwrap();
      let older = create(&db, &watch()).await.unwrap();
      let newer = create(&db, &watch()).await.unwrap();
      let mut sell = watch();
      sell.direction = WatchDirection::Sell;
      let other_direction = create(&db, &sell).await.unwrap();
      let mut jita = watch();
      jita.location_id = Some(60_003_761);
      let other_market = create(&db, &jita).await.unwrap();
      stamp(&db, older.id, "2026-07-01T00:00:00Z").await;
      stamp(&db, newer.id, "2026-07-02T00:00:00Z").await;

      let dedup = MIGRATION.split_once(';').unwrap().0;
      sqlx::query(dedup).execute(db.writer()).await.unwrap();

      let mut survivors: Vec<i64> = list(&db).await.unwrap().iter().map(|row| row.id).collect();
      survivors.sort_unstable();
      let mut expected = vec![newer.id, other_direction.id, other_market.id];
      expected.sort_unstable();
      assert_eq!(survivors, expected);
    }

    #[tokio::test]
    async fn it_breaks_an_updated_at_tie_by_the_higher_id() {
      let db = store::open_test().await.unwrap();
      let first = create(&db, &watch()).await.unwrap();
      let second = create(&db, &watch()).await.unwrap();
      stamp(&db, first.id, "2026-07-01T00:00:00Z").await;
      stamp(&db, second.id, "2026-07-01T00:00:00Z").await;

      let dedup = MIGRATION.split_once(';').unwrap().0;
      sqlx::query(dedup).execute(db.writer()).await.unwrap();

      let survivors: Vec<i64> = list(&db).await.unwrap().iter().map(|row| row.id).collect();
      assert_eq!(survivors, vec![second.id]);
    }
  }
}
