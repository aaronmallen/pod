use crate::{
  services::location_search::LocationTier,
  store::{
    Database, Error,
    model::{MarketComparisonPin, MarketComparisonPinMarket},
  },
};

pub async fn create(db: &Database, type_id: i64) -> Result<MarketComparisonPin, Error> {
  let pin = sqlx::query_as::<_, MarketComparisonPin>(
    "INSERT INTO market_comparison_pin (type_id, position) \
    VALUES (?, (SELECT COALESCE(MAX(position), 0) + 1 FROM market_comparison_pin)) \
    RETURNING id, position, type_id",
  )
  .bind(type_id)
  .fetch_one(db.writer())
  .await?;
  Ok(pin)
}

pub async fn list(db: &Database) -> Result<Vec<MarketComparisonPin>, Error> {
  let rows = sqlx::query_as::<_, MarketComparisonPin>(
    "SELECT id, position, type_id FROM market_comparison_pin ORDER BY position, id",
  )
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

pub async fn delete(db: &Database, pin_id: i64) -> Result<u64, Error> {
  let result = sqlx::query("DELETE FROM market_comparison_pin WHERE id = ?")
    .bind(pin_id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn reorder(db: &Database, ordered_ids: &[i64]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;
  for (position, id) in ordered_ids.iter().enumerate() {
    sqlx::query("UPDATE market_comparison_pin SET position = ? WHERE id = ?")
      .bind(position as i64)
      .bind(id)
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;
  Ok(())
}

pub async fn add_market(
  db: &Database,
  pin_id: i64,
  place_id: i64,
  tier: LocationTier,
) -> Result<MarketComparisonPinMarket, Error> {
  let market = sqlx::query_as::<_, MarketComparisonPinMarket>(
    "INSERT INTO market_comparison_pin_market (pin_id, place_id, tier, position) \
    VALUES (?, ?, ?, (SELECT COALESCE(MAX(position), 0) + 1 FROM market_comparison_pin_market WHERE pin_id = ?)) \
    RETURNING id, pin_id, place_id, position, tier",
  )
  .bind(pin_id)
  .bind(place_id)
  .bind(tier.as_str())
  .bind(pin_id)
  .fetch_one(db.writer())
  .await?;
  Ok(market)
}

pub async fn remove_market(db: &Database, market_id: i64) -> Result<u64, Error> {
  let result = sqlx::query("DELETE FROM market_comparison_pin_market WHERE id = ?")
    .bind(market_id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected())
}

#[allow(dead_code)]
pub async fn reorder_markets(db: &Database, ordered_ids: &[i64]) -> Result<(), Error> {
  let mut tx = db.writer().begin().await?;
  for (position, id) in ordered_ids.iter().enumerate() {
    sqlx::query("UPDATE market_comparison_pin_market SET position = ? WHERE id = ?")
      .bind(position as i64)
      .bind(id)
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;
  Ok(())
}

pub async fn markets(db: &Database, pin_id: i64) -> Result<Vec<MarketComparisonPinMarket>, Error> {
  let rows = sqlx::query_as::<_, MarketComparisonPinMarket>(
    "SELECT id, pin_id, place_id, position, tier FROM market_comparison_pin_market \
    WHERE pin_id = ? ORDER BY position, id",
  )
  .bind(pin_id)
  .fetch_all(db.reader())
  .await?;
  Ok(rows)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  async fn market_places(db: &Database, pin_id: i64) -> Vec<i64> {
    markets(db, pin_id)
      .await
      .unwrap()
      .into_iter()
      .map(|m| m.place_id)
      .collect()
  }

  mod pins {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_creates_pins_appended_at_the_tail() {
      let db = store::open_test().await.unwrap();

      let first = create(&db, 34).await.unwrap();
      let second = create(&db, 35).await.unwrap();

      assert_eq!(first.position, 1);
      assert_eq!(second.position, 2);
      let listed: Vec<(i64, i64)> = list(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|p| (p.id, p.type_id))
        .collect();
      assert_eq!(listed, vec![(first.id, 34), (second.id, 35)]);
    }

    #[tokio::test]
    async fn it_allows_duplicate_pins_of_the_same_type() {
      let db = store::open_test().await.unwrap();

      let first = create(&db, 34).await.unwrap();
      let second = create(&db, 34).await.unwrap();

      assert_ne!(first.id, second.id);
      let types: Vec<i64> = list(&db).await.unwrap().into_iter().map(|p| p.type_id).collect();
      assert_eq!(types, vec![34, 34]);
    }

    #[tokio::test]
    async fn it_deletes_a_pin_and_cascades_its_markets() {
      let db = store::open_test().await.unwrap();
      let pin = create(&db, 34).await.unwrap();
      add_market(&db, pin.id, 60_003_760, LocationTier::Station)
        .await
        .unwrap();
      add_market(&db, pin.id, 10_000_002, LocationTier::Region).await.unwrap();

      let affected = delete(&db, pin.id).await.unwrap();

      assert_eq!(affected, 1);
      assert!(list(&db).await.unwrap().is_empty());
      let orphans = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM market_comparison_pin_market")
        .fetch_one(db.reader())
        .await
        .unwrap();
      assert_eq!(orphans, 0);
    }

    #[tokio::test]
    async fn it_persists_a_full_batch_reorder() {
      let db = store::open_test().await.unwrap();
      let first = create(&db, 34).await.unwrap();
      let second = create(&db, 35).await.unwrap();
      let third = create(&db, 36).await.unwrap();

      reorder(&db, &[third.id, first.id, second.id]).await.unwrap();

      let ids: Vec<i64> = list(&db).await.unwrap().into_iter().map(|p| p.id).collect();
      assert_eq!(ids, vec![third.id, first.id, second.id]);
    }

    #[tokio::test]
    async fn it_keeps_appending_at_the_tail_after_a_removal() {
      let db = store::open_test().await.unwrap();
      let first = create(&db, 34).await.unwrap();
      let second = create(&db, 35).await.unwrap();
      delete(&db, first.id).await.unwrap();

      let third = create(&db, 36).await.unwrap();

      assert_eq!(third.position, 3);
      let ids: Vec<i64> = list(&db).await.unwrap().into_iter().map(|p| p.id).collect();
      assert_eq!(ids, vec![second.id, third.id]);
    }
  }

  mod markets {
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn it_adds_markets_appended_at_the_tail_per_pin() {
      let db = store::open_test().await.unwrap();
      let pin = create(&db, 34).await.unwrap();
      let other = create(&db, 35).await.unwrap();

      let first = add_market(&db, pin.id, 60_003_760, LocationTier::Station)
        .await
        .unwrap();
      let second = add_market(&db, pin.id, 10_000_002, LocationTier::Region).await.unwrap();
      let elsewhere = add_market(&db, other.id, 30_000_142, LocationTier::System)
        .await
        .unwrap();

      assert_eq!(first.position, 1);
      assert_eq!(second.position, 2);
      assert_eq!(elsewhere.position, 1);
      assert_eq!(market_places(&db, pin.id).await, vec![60_003_760, 10_000_002]);
      assert_eq!(market_places(&db, other.id).await, vec![30_000_142]);
    }

    #[tokio::test]
    async fn it_round_trips_the_tier_string() {
      let db = store::open_test().await.unwrap();
      let pin = create(&db, 34).await.unwrap();

      add_market(&db, pin.id, 10_000_002, LocationTier::Region).await.unwrap();

      let tiers: Vec<Option<LocationTier>> = markets(&db, pin.id)
        .await
        .unwrap()
        .into_iter()
        .map(|m| LocationTier::parse(&m.tier))
        .collect();
      assert_eq!(tiers, vec![Some(LocationTier::Region)]);
    }

    #[tokio::test]
    async fn it_removes_a_market() {
      let db = store::open_test().await.unwrap();
      let pin = create(&db, 34).await.unwrap();
      let first = add_market(&db, pin.id, 60_003_760, LocationTier::Station)
        .await
        .unwrap();
      add_market(&db, pin.id, 60_008_494, LocationTier::Station)
        .await
        .unwrap();

      let affected = remove_market(&db, first.id).await.unwrap();

      assert_eq!(affected, 1);
      assert_eq!(market_places(&db, pin.id).await, vec![60_008_494]);
    }

    #[tokio::test]
    async fn it_keeps_appending_at_the_tail_after_a_removal() {
      let db = store::open_test().await.unwrap();
      let pin = create(&db, 34).await.unwrap();
      let first = add_market(&db, pin.id, 60_003_760, LocationTier::Station)
        .await
        .unwrap();
      add_market(&db, pin.id, 60_008_494, LocationTier::Station)
        .await
        .unwrap();
      remove_market(&db, first.id).await.unwrap();

      let third = add_market(&db, pin.id, 60_004_588, LocationTier::Station)
        .await
        .unwrap();

      assert_eq!(third.position, 3);
      assert_eq!(market_places(&db, pin.id).await, vec![60_008_494, 60_004_588]);
    }

    #[tokio::test]
    async fn it_persists_a_full_batch_reorder() {
      let db = store::open_test().await.unwrap();
      let pin = create(&db, 34).await.unwrap();
      let first = add_market(&db, pin.id, 60_003_760, LocationTier::Station)
        .await
        .unwrap();
      let second = add_market(&db, pin.id, 60_008_494, LocationTier::Station)
        .await
        .unwrap();
      let third = add_market(&db, pin.id, 60_004_588, LocationTier::Station)
        .await
        .unwrap();

      reorder_markets(&db, &[second.id, third.id, first.id]).await.unwrap();

      assert_eq!(
        market_places(&db, pin.id).await,
        vec![60_008_494, 60_004_588, 60_003_760]
      );
    }
  }
}
