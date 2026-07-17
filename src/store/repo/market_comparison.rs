use crate::{
  services::location_search::LocationTier,
  store::{Database, Error},
};

const DEFAULT_MARKETS: [(i64, LocationTier); 3] = [
  (60_003_760, LocationTier::Station),
  (60_008_494, LocationTier::Station),
  (60_004_588, LocationTier::Station),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComparisonMarket {
  pub place_id: i64,
  pub tier: LocationTier,
}

#[allow(dead_code)]
pub async fn list(db: &Database) -> Result<Vec<ComparisonMarket>, Error> {
  let rows = fetch_all(db).await?;
  if !rows.is_empty() {
    return Ok(rows);
  }
  seed_defaults(db).await?;
  fetch_all(db).await
}

#[allow(dead_code)]
pub async fn add(db: &Database, place_id: i64, tier: LocationTier) -> Result<(), Error> {
  sqlx::query("INSERT INTO market_comparison (place_id, tier) VALUES (?, ?) ON CONFLICT(place_id) DO NOTHING")
    .bind(place_id)
    .bind(tier.as_str())
    .execute(db.writer())
    .await?;
  Ok(())
}

#[allow(dead_code)]
pub async fn remove(db: &Database, place_id: i64) -> Result<bool, Error> {
  let remaining = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM market_comparison")
    .fetch_one(db.reader())
    .await?;
  if remaining <= 1 {
    return Ok(false);
  }
  let result = sqlx::query("DELETE FROM market_comparison WHERE place_id = ?")
    .bind(place_id)
    .execute(db.writer())
    .await?;
  Ok(result.rows_affected() > 0)
}

async fn fetch_all(db: &Database) -> Result<Vec<ComparisonMarket>, Error> {
  let rows = sqlx::query_as::<_, (i64, String)>("SELECT place_id, tier FROM market_comparison ORDER BY id")
    .fetch_all(db.reader())
    .await?;
  Ok(
    rows
      .into_iter()
      .map(|(place_id, tier)| ComparisonMarket {
        place_id,
        tier: parse_tier(&tier, place_id),
      })
      .collect(),
  )
}

async fn seed_defaults(db: &Database) -> Result<(), Error> {
  for (place_id, tier) in DEFAULT_MARKETS {
    add(db, place_id, tier).await?;
  }
  Ok(())
}

fn parse_tier(value: &str, place_id: i64) -> LocationTier {
  LocationTier::parse(value)
    .or_else(|| LocationTier::from_id(place_id))
    .unwrap_or(LocationTier::Station)
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;
  use crate::store;

  #[tokio::test]
  async fn it_adds_and_lists_back_a_market() {
    let db = store::open_test().await.unwrap();
    add(&db, 10_000_002, LocationTier::Region).await.unwrap();

    let markets = fetch_all(&db).await.unwrap();

    assert_eq!(
      markets,
      vec![ComparisonMarket {
        place_id: 10_000_002,
        tier: LocationTier::Region
      }]
    );
  }

  #[tokio::test]
  async fn it_preserves_insertion_order() {
    let db = store::open_test().await.unwrap();
    add(&db, 60_003_760, LocationTier::Station).await.unwrap();
    add(&db, 10_000_043, LocationTier::Region).await.unwrap();
    add(&db, 30_000_142, LocationTier::System).await.unwrap();

    let ids: Vec<i64> = fetch_all(&db).await.unwrap().into_iter().map(|m| m.place_id).collect();

    assert_eq!(ids, vec![60_003_760, 10_000_043, 30_000_142]);
  }

  #[tokio::test]
  async fn it_is_idempotent_on_re_add() {
    let db = store::open_test().await.unwrap();
    add(&db, 60_003_760, LocationTier::Station).await.unwrap();
    add(&db, 60_003_760, LocationTier::Station).await.unwrap();

    assert_eq!(fetch_all(&db).await.unwrap().len(), 1);
  }

  #[tokio::test]
  async fn it_removes_a_market() {
    let db = store::open_test().await.unwrap();
    add(&db, 60_003_760, LocationTier::Station).await.unwrap();
    add(&db, 60_008_494, LocationTier::Station).await.unwrap();

    let removed = remove(&db, 60_003_760).await.unwrap();

    assert!(removed);
    let ids: Vec<i64> = fetch_all(&db).await.unwrap().into_iter().map(|m| m.place_id).collect();
    assert_eq!(ids, vec![60_008_494]);
  }

  #[tokio::test]
  async fn it_refuses_to_remove_the_last_market() {
    let db = store::open_test().await.unwrap();
    add(&db, 60_003_760, LocationTier::Station).await.unwrap();

    let removed = remove(&db, 60_003_760).await.unwrap();

    assert!(!removed);
    assert_eq!(fetch_all(&db).await.unwrap().len(), 1);
  }

  #[tokio::test]
  async fn it_seeds_the_default_markets_when_empty() {
    let db = store::open_test().await.unwrap();

    let markets = list(&db).await.unwrap();

    assert_eq!(
      markets,
      vec![
        ComparisonMarket {
          place_id: 60_003_760,
          tier: LocationTier::Station
        },
        ComparisonMarket {
          place_id: 60_008_494,
          tier: LocationTier::Station
        },
        ComparisonMarket {
          place_id: 60_004_588,
          tier: LocationTier::Station
        },
      ]
    );
  }

  #[tokio::test]
  async fn it_does_not_reseed_a_non_empty_set() {
    let db = store::open_test().await.unwrap();
    add(&db, 10_000_002, LocationTier::Region).await.unwrap();

    let markets = list(&db).await.unwrap();

    assert_eq!(
      markets,
      vec![ComparisonMarket {
        place_id: 10_000_002,
        tier: LocationTier::Region
      }]
    );
  }
}
