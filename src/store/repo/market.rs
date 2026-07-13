use crate::store::{Database, Error};

const DEFAULT_ID: i64 = 1;

#[allow(dead_code)]
pub async fn default_market(db: &Database) -> Result<Option<i64>, Error> {
  Ok(
    sqlx::query_scalar::<_, i64>("SELECT place_id FROM market_default WHERE id = ?")
      .bind(DEFAULT_ID)
      .fetch_optional(db.reader())
      .await?,
  )
}

#[allow(dead_code)]
pub async fn set_default_market(db: &Database, place_id: i64) -> Result<(), Error> {
  sqlx::query(
    "INSERT INTO market_default (id, place_id) VALUES (?, ?) \
    ON CONFLICT(id) DO UPDATE SET place_id = excluded.place_id",
  )
  .bind(DEFAULT_ID)
  .bind(place_id)
  .execute(db.writer())
  .await?;
  Ok(())
}

#[allow(dead_code)]
pub async fn clear_default_market(db: &Database) -> Result<(), Error> {
  sqlx::query("DELETE FROM market_default WHERE id = ?")
    .bind(DEFAULT_ID)
    .execute(db.writer())
    .await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::store;

  #[tokio::test]
  async fn it_returns_none_for_an_unset_default_market() {
    let db = store::open_test().await.unwrap();

    let market = default_market(&db).await.unwrap();

    assert_eq!(market, None);
  }

  #[tokio::test]
  async fn it_sets_and_reads_a_default_market() {
    let db = store::open_test().await.unwrap();

    set_default_market(&db, 10_000_002).await.unwrap();

    assert_eq!(default_market(&db).await.unwrap(), Some(10_000_002));
  }

  #[tokio::test]
  async fn it_overwrites_a_default_market_on_set() {
    let db = store::open_test().await.unwrap();

    set_default_market(&db, 10_000_002).await.unwrap();
    set_default_market(&db, 10_000_043).await.unwrap();

    assert_eq!(default_market(&db).await.unwrap(), Some(10_000_043));
  }

  #[tokio::test]
  async fn it_clears_a_default_market_back_to_unset() {
    let db = store::open_test().await.unwrap();
    set_default_market(&db, 10_000_002).await.unwrap();

    clear_default_market(&db).await.unwrap();

    assert_eq!(default_market(&db).await.unwrap(), None);
  }
}
