use crate::store::{
  Database, Error,
  model::{MarketWatch, NewWatch},
};

#[allow(dead_code)]
pub async fn list(db: &Database) -> Result<Vec<MarketWatch>, Error> {
  let rows = sqlx::query_as::<_, MarketWatch>(
    "SELECT character_id, created_at, direction, id, location_id, location_tier, region_id, target_price, type_id, updated_at \
    FROM market_watchlist ORDER BY sort_order ASC, id DESC",
  )
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn list_for_character(db: &Database, character_id: i64) -> Result<Vec<MarketWatch>, Error> {
  let rows = sqlx::query_as::<_, MarketWatch>(
    "SELECT character_id, created_at, direction, id, location_id, location_tier, region_id, target_price, type_id, updated_at \
    FROM market_watchlist WHERE character_id = ? ORDER BY sort_order ASC, id DESC",
  )
  .bind(character_id)
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

#[allow(dead_code)]
pub async fn get(db: &Database, id: i64) -> Result<Option<MarketWatch>, Error> {
  let row = sqlx::query_as::<_, MarketWatch>(
    "SELECT character_id, created_at, direction, id, location_id, location_tier, region_id, target_price, type_id, updated_at \
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
      (character_id, created_at, direction, location_id, location_tier, region_id, sort_order, target_price, type_id, \
      updated_at) \
    VALUES (?, ?, ?, ?, ?, ?, (SELECT COALESCE(MIN(sort_order), 1) - 1 FROM market_watchlist), ?, ?, ?) \
    RETURNING character_id, created_at, direction, id, location_id, location_tier, region_id, target_price, type_id, \
      updated_at",
  )
  .bind(input.character_id)
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

#[allow(dead_code)]
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
  use crate::store::{
    self,
    model::{Alliance, Bloodline, Character, Corporation, Gender, Race, WatchDirection},
    repo::character,
  };

  const PILOT: i64 = 90_000_001;
  const OTHER: i64 = 90_000_002;

  async fn seed_character(db: &Database, id: i64) {
    let corp_id = 98_000_001;
    let alliance_id = 99_000_001;
    let alliance = Alliance::new(alliance_id, corp_id, id, "2003-01-01", "Test Alliance", "TST");
    let race = Race::new(2, alliance_id, "A race.", "Caldari");
    let mut corp = Corporation::new(corp_id, "Test Corp", "TSC");
    corp.set_ceo_id(id);
    corp.set_creator_id(id);
    corp.set_member_count(1);
    corp.set_tax_rate(0.0);
    let bloodline = Bloodline::new(1, corp_id, 2, 3, "A bloodline.", 4, 5, "Civire", 4, 4);
    let character = Character::new(id, 1, corp_id, 2, "2003-05-12", Gender::Male, "Pilot");
    character::insert_with_org(db, &character, &bloodline, &race, &corp, Some(&alliance), None)
      .await
      .unwrap();
  }

  fn watch(character_id: i64) -> NewWatch {
    NewWatch {
      character_id,
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
      seed_character(&db, PILOT).await;

      let created = create(&db, &watch(PILOT)).await.unwrap();

      assert!(created.id > 0);
      assert_eq!(created.character_id, PILOT);
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
      seed_character(&db, PILOT).await;
      let created = create(&db, &watch(PILOT)).await.unwrap();

      let mut edit = watch(PILOT);
      edit.direction = WatchDirection::Sell;
      edit.target_price = Some(9_000_000.0);
      edit.location_id = None;
      let affected = update(&db, created.id, &edit).await.unwrap();

      assert_eq!(affected, 1);
      let fetched = get(&db, created.id).await.unwrap().unwrap();
      assert_eq!(fetched.direction, "sell");
      assert_eq!(fetched.target_price, Some(9_000_000.0));
      assert_eq!(fetched.location_id, None);
      assert_eq!(fetched.character_id, PILOT);
    }

    #[tokio::test]
    async fn it_lists_newest_first_and_scopes_by_character() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;
      seed_character(&db, OTHER).await;
      let first = create(&db, &watch(PILOT)).await.unwrap();
      let second = create(&db, &watch(PILOT)).await.unwrap();
      let other = create(&db, &watch(OTHER)).await.unwrap();

      let all = list(&db).await.unwrap();
      assert_eq!(all.len(), 3);
      assert_eq!(all[0].id, other.id);

      let mine = list_for_character(&db, PILOT).await.unwrap();
      assert_eq!(mine.len(), 2);
      assert_eq!(mine[0].id, second.id);
      assert_eq!(mine[1].id, first.id);
    }

    #[tokio::test]
    async fn it_deletes_a_watch() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;
      let created = create(&db, &watch(PILOT)).await.unwrap();

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
      seed_character(&db, PILOT).await;
      let first = create(&db, &watch(PILOT)).await.unwrap();
      let second = create(&db, &watch(PILOT)).await.unwrap();
      let third = create(&db, &watch(PILOT)).await.unwrap();

      assert_eq!(ids(&db).await, vec![third.id, second.id, first.id]);
    }

    #[tokio::test]
    async fn it_persists_a_full_batch_reorder() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;
      let first = create(&db, &watch(PILOT)).await.unwrap();
      let second = create(&db, &watch(PILOT)).await.unwrap();
      let third = create(&db, &watch(PILOT)).await.unwrap();

      reorder(&db, &[first.id, third.id, second.id]).await.unwrap();

      assert_eq!(ids(&db).await, vec![first.id, third.id, second.id]);
      let mine = list_for_character(&db, PILOT).await.unwrap();
      assert_eq!(
        mine.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![first.id, third.id, second.id]
      );
    }

    #[tokio::test]
    async fn it_keeps_inserting_at_the_top_after_a_reorder() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;
      let first = create(&db, &watch(PILOT)).await.unwrap();
      let second = create(&db, &watch(PILOT)).await.unwrap();
      reorder(&db, &[first.id, second.id]).await.unwrap();

      let third = create(&db, &watch(PILOT)).await.unwrap();

      assert_eq!(ids(&db).await, vec![third.id, first.id, second.id]);
    }

    #[tokio::test]
    async fn it_backfills_from_created_at_desc_ordering() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;
      let first = create(&db, &watch(PILOT)).await.unwrap();
      let second = create(&db, &watch(PILOT)).await.unwrap();
      let third = create(&db, &watch(PILOT)).await.unwrap();
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

  mod cascade {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_drops_watch_rows_when_the_character_is_deleted() {
      let db = store::open_test().await.unwrap();
      seed_character(&db, PILOT).await;
      let created = create(&db, &watch(PILOT)).await.unwrap();

      character::delete(&db, PILOT).await.unwrap();

      assert_eq!(get(&db, created.id).await.unwrap(), None);
      assert!(list_for_character(&db, PILOT).await.unwrap().is_empty());
    }
  }
}
