//! Repository for intraday price observations and daily OHLC aggregates.

use std::collections::HashMap;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use sea_orm::{
  ActiveValue, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, FromQueryResult, Order,
  QueryFilter, QueryOrder, Statement,
};

use crate::{
  Error,
  entities::{
    character_asset::{Column as AssetColumn, Entity as AssetEntity},
    type_price::{ActiveModel as PriceActive, Column as PriceColumn, Entity as PriceEntity},
    type_price_history::{ActiveModel as HistoryActive, Column as HistoryColumn, Entity as HistoryEntity},
  },
};

#[derive(Debug, FromQueryResult)]
struct TypeIdRow {
  type_id: i32,
}


#[derive(Debug, FromQueryResult)]
struct DateRow {
  date: String,
}

#[derive(Debug, FromQueryResult)]
struct PriceSummaryRow {
  open: f64,
  high: f64,
  low: f64,
  close: f64,
  avg: f64,
  sample_count: i32,
}

/// Repository for type price intraday observations and daily OHLC aggregation.
pub struct Repo<'a> {
  db: &'a DatabaseConnection,
}

impl<'a> Repo<'a> {
  /// Creates a new `Repo` bound to the given database connection.
  pub fn new(db: &'a DatabaseConnection) -> Self {
    Self {
      db,
    }
  }

  /// Inserts a new intraday price observation for `type_id`.
  pub async fn insert_price(&self, type_id: i32, price: f64, fetched_at: DateTime<Utc>) -> Result<(), Error> {
    let active = PriceActive {
      id: ActiveValue::NotSet,
      type_id: ActiveValue::Set(type_id),
      price: ActiveValue::Set(price),
      fetched_at: ActiveValue::Set(fetched_at.to_rfc3339()),
    };
    PriceEntity::insert(active).exec(self.db).await?;
    Ok(())
  }

  /// Returns the most recent price for `type_id`.
  ///
  /// Checks `type_prices` first (intraday rows), falling back to the most
  /// recent `type_price_histories.close` when no intraday row exists.
  pub async fn latest_price(&self, type_id: i32) -> Result<Option<f64>, Error> {
    let intraday = PriceEntity::find()
      .filter(PriceColumn::TypeId.eq(type_id))
      .order_by(PriceColumn::FetchedAt, Order::Desc)
      .one(self.db)
      .await?;

    if let Some(row) = intraday {
      return Ok(Some(row.price));
    }

    let history = HistoryEntity::find()
      .filter(HistoryColumn::TypeId.eq(type_id))
      .order_by(HistoryColumn::Date, Order::Desc)
      .one(self.db)
      .await?;

    Ok(history.map(|h| h.close))
  }

  /// Returns all distinct type IDs that should have prices tracked.
  ///
  /// The set is the UNION of type IDs present in `character_assets` and
  /// `type_price_histories`.
  pub async fn types_to_track(&self) -> Result<Vec<i32>, Error> {
    let sql = "
      SELECT type_id FROM character_assets
      UNION
      SELECT type_id FROM type_price_histories
    ";
    let rows = TypeIdRow::find_by_statement(Statement::from_string(DbBackend::Sqlite, sql))
      .all(self.db)
      .await?;
    Ok(rows.into_iter().map(|r| r.type_id).collect())
  }

  /// Aggregates all intraday `type_prices` rows for `date` into OHLC history
  /// records and deletes the processed rows.
  ///
  /// For each type that has rows on `date`, computes open (first fetched),
  /// high (max), low (min), close (last fetched), avg (mean), and
  /// sample_count, then upserts into `type_price_histories`. Processed
  /// intraday rows are deleted after the upsert.
  pub async fn aggregate_and_prune(&self, date: NaiveDate) -> Result<(), Error> {
    let date_str = date.format("%Y-%m-%d").to_string();

    let type_ids_sql = format!("SELECT DISTINCT type_id FROM type_prices WHERE fetched_at LIKE '{date_str}%'");
    let type_id_rows = TypeIdRow::find_by_statement(Statement::from_string(DbBackend::Sqlite, type_ids_sql))
      .all(self.db)
      .await?;

    for row in type_id_rows {
      let tid = row.type_id;

      let summary_sql = format!(
        "
        SELECT
          (SELECT price FROM type_prices WHERE type_id = {tid} AND fetched_at LIKE '{date_str}%' ORDER BY fetched_at ASC LIMIT 1) AS open,
          MAX(price) AS high,
          MIN(price) AS low,
          (SELECT price FROM type_prices WHERE type_id = {tid} AND fetched_at LIKE '{date_str}%' ORDER BY fetched_at DESC LIMIT 1) AS close,
          AVG(price) AS avg,
          COUNT(*) AS sample_count
        FROM type_prices
        WHERE type_id = {tid} AND fetched_at LIKE '{date_str}%'
        "
      );

      let summaries = PriceSummaryRow::find_by_statement(Statement::from_string(DbBackend::Sqlite, summary_sql))
        .all(self.db)
        .await?;

      let Some(summary) = summaries.into_iter().next() else {
        continue;
      };

      let active = HistoryActive {
        id: ActiveValue::NotSet,
        type_id: ActiveValue::Set(tid),
        date: ActiveValue::Set(date_str.clone()),
        open: ActiveValue::Set(summary.open),
        high: ActiveValue::Set(summary.high),
        low: ActiveValue::Set(summary.low),
        close: ActiveValue::Set(summary.close),
        avg: ActiveValue::Set(summary.avg),
        sample_count: ActiveValue::Set(summary.sample_count),
      };

      let upsert_sql = format!(
        "
        INSERT INTO type_price_histories (type_id, date, open, high, low, close, avg, sample_count)
        VALUES ({}, '{}', {}, {}, {}, {}, {}, {})
        ON CONFLICT (type_id, date) DO UPDATE SET
          open = excluded.open,
          high = excluded.high,
          low = excluded.low,
          close = excluded.close,
          avg = excluded.avg,
          sample_count = excluded.sample_count
        ",
        tid, date_str, summary.open, summary.high, summary.low, summary.close, summary.avg, summary.sample_count,
      );
      let _ = active;
      self.db.execute_unprepared(&upsert_sql).await?;

      let delete_sql = format!("DELETE FROM type_prices WHERE type_id = {tid} AND fetched_at LIKE '{date_str}%'");
      self.db.execute_unprepared(&delete_sql).await?;
    }

    Ok(())
  }

  /// Returns NAV history for the given character IDs going back `days` calendar days,
  /// plus a synthetic data point for today using the latest intraday prices.
  ///
  /// Computes NAV = sum(price * quantity) per day, sorted ascending. Returns an
  /// empty vec if fewer than 2 data points exist.
  pub async fn nav_history(&self, char_ids: &[i64], days: u32) -> Result<Vec<(NaiveDate, f64)>, Error> {
    if char_ids.is_empty() {
      return Ok(Vec::new());
    }

    let assets = AssetEntity::find()
      .filter(AssetColumn::CharacterId.is_in(char_ids.to_vec()))
      .all(self.db)
      .await?;

    if assets.is_empty() {
      return Ok(Vec::new());
    }

    let mut qty_map: HashMap<i32, i64> = HashMap::new();
    for asset in &assets {
      *qty_map.entry(asset.type_id).or_insert(0) += asset.quantity as i64;
    }
    let tracked: Vec<i32> = qty_map.keys().copied().collect();

    let cutoff_str = (Utc::now().date_naive() - Duration::days(days as i64))
      .format("%Y-%m-%d")
      .to_string();

    let histories = HistoryEntity::find()
      .filter(HistoryColumn::TypeId.is_in(tracked.clone()))
      .filter(HistoryColumn::Date.gte(cutoff_str))
      .all(self.db)
      .await?;

    let mut nav_by_date: HashMap<NaiveDate, f64> = HashMap::new();
    for h in &histories {
      if let (Ok(date), Some(&qty)) = (NaiveDate::parse_from_str(&h.date, "%Y-%m-%d"), qty_map.get(&h.type_id)) {
        *nav_by_date.entry(date).or_insert(0.0) += h.close * qty as f64;
      }
    }

    // Synthetic today point from latest intraday prices.
    let intraday = PriceEntity::find()
      .filter(PriceColumn::TypeId.is_in(tracked))
      .order_by(PriceColumn::FetchedAt, Order::Desc)
      .all(self.db)
      .await?;

    let mut latest_prices: HashMap<i32, f64> = HashMap::new();
    for p in intraday {
      latest_prices.entry(p.type_id).or_insert(p.price);
    }

    let today_nav: f64 = latest_prices
      .iter()
      .filter_map(|(tid, price)| qty_map.get(tid).map(|&qty| price * qty as f64))
      .sum();

    if today_nav > 0.0 {
      nav_by_date.insert(Utc::now().date_naive(), today_nav);
    }

    let mut result: Vec<(NaiveDate, f64)> = nav_by_date.into_iter().collect();
    result.sort_by_key(|(d, _)| *d);

    if result.len() < 2 {
      result.clear();
    }

    Ok(result)
  }

  /// Returns distinct UTC calendar dates that have intraday rows older than
  /// `before_date` (used to discover which dates need EOD aggregation).
  pub async fn dates_needing_aggregation(&self, before_date: NaiveDate) -> Result<Vec<NaiveDate>, Error> {
    let before_str = before_date.format("%Y-%m-%d").to_string();
    let sql = format!(
      "SELECT DISTINCT substr(fetched_at, 1, 10) AS date FROM type_prices WHERE substr(fetched_at, 1, 10) < '{before_str}'"
    );
    let rows = DateRow::find_by_statement(Statement::from_string(DbBackend::Sqlite, sql))
      .all(self.db)
      .await?;
    let mut dates = Vec::new();
    for r in rows {
      if let Ok(d) = NaiveDate::parse_from_str(&r.date, "%Y-%m-%d") {
        dates.push(d);
      }
    }
    Ok(dates)
  }
}

#[cfg(test)]
mod tests {
  use chrono::{TimeZone, Utc};
  use sea_orm::{Database, DatabaseConnection};

  use super::*;

  async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    crate::migrations::run(&db).await.unwrap();
    db
  }

  fn utc(year: i32, month: u32, day: u32, h: u32, m: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, h, m, s).unwrap()
  }

  mod latest_price {
    use super::*;

    #[tokio::test]
    async fn returns_none_when_no_data() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.latest_price(34).await.unwrap();
      assert!(result.is_none());
    }

    #[tokio::test]
    async fn returns_intraday_price_when_present() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let ts = utc(2025, 1, 1, 12, 0, 0);
      repo.insert_price(34, 5.5, ts).await.unwrap();

      let result = repo.latest_price(34).await.unwrap();
      assert_eq!(result, Some(5.5));
    }

    #[tokio::test]
    async fn returns_most_recent_intraday_when_multiple_exist() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo.insert_price(34, 5.5, utc(2025, 1, 1, 10, 0, 0)).await.unwrap();
      repo.insert_price(34, 6.0, utc(2025, 1, 1, 12, 0, 0)).await.unwrap();
      repo.insert_price(34, 5.8, utc(2025, 1, 1, 11, 0, 0)).await.unwrap();

      let result = repo.latest_price(34).await.unwrap();
      assert_eq!(result, Some(6.0));
    }

    #[tokio::test]
    async fn falls_back_to_history_when_no_intraday() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      use sea_orm::ActiveValue::{NotSet, Set};

      use crate::entities::type_price_history::{ActiveModel as HistActive, Entity as HistEntity};
      HistEntity::insert(HistActive {
        id: NotSet,
        type_id: Set(34),
        date: Set("2025-01-01".to_string()),
        open: Set(5.0),
        high: Set(6.0),
        low: Set(4.5),
        close: Set(5.9),
        avg: Set(5.4),
        sample_count: Set(10),
      })
      .exec(&db)
      .await
      .unwrap();

      let result = repo.latest_price(34).await.unwrap();
      assert_eq!(result, Some(5.9));
    }

    #[tokio::test]
    async fn returns_most_recent_history_close_when_multiple() {
      let db = setup_db().await;
      use sea_orm::ActiveValue::Set;

      use crate::entities::type_price_history::{ActiveModel as HistActive, Entity as HistEntity};

      for (date, close) in [("2025-01-01", 5.0_f64), ("2025-01-03", 7.0), ("2025-01-02", 6.0)] {
        use sea_orm::ActiveValue::NotSet;
        HistEntity::insert(HistActive {
          id: NotSet,
          type_id: Set(34),
          date: Set(date.to_string()),
          open: Set(close),
          high: Set(close),
          low: Set(close),
          close: Set(close),
          avg: Set(close),
          sample_count: Set(1),
        })
        .exec(&db)
        .await
        .unwrap();
      }

      let repo = Repo::new(&db);
      let result = repo.latest_price(34).await.unwrap();
      assert_eq!(result, Some(7.0));
    }
  }

  mod nav_history {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_char_ids_is_empty() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let result = repo.nav_history(&[], 30).await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_empty_when_fewer_than_two_data_points() {
      let db = setup_db().await;
      insert_character_and_asset(&db, 1, 34, 100).await;

      use sea_orm::ActiveValue::{NotSet, Set};

      use crate::entities::type_price_history::{ActiveModel as HistActive, Entity as HistEntity};
      HistEntity::insert(HistActive {
        id: NotSet,
        type_id: Set(34),
        date: Set("2099-01-01".to_string()),
        open: Set(5.0),
        high: Set(5.0),
        low: Set(5.0),
        close: Set(5.0),
        avg: Set(5.0),
        sample_count: Set(1),
      })
      .exec(&db)
      .await
      .unwrap();

      let repo = Repo::new(&db);
      let result = repo.nav_history(&[1], 36500).await.unwrap();
      assert!(result.is_empty());
    }
  }

  mod dates_needing_aggregation {
    use super::*;

    #[tokio::test]
    async fn returns_empty_when_no_intraday_rows() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let before = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
      let result = repo.dates_needing_aggregation(before).await.unwrap();
      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn returns_dates_strictly_before_cutoff() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      repo.insert_price(34, 5.0, utc(2025, 1, 1, 12, 0, 0)).await.unwrap();
      repo.insert_price(34, 5.5, utc(2025, 1, 3, 12, 0, 0)).await.unwrap();

      let before = NaiveDate::from_ymd_opt(2025, 1, 3).unwrap();
      let result = repo.dates_needing_aggregation(before).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0], NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
    }
  }

  mod insert_price {
    use super::*;

    #[tokio::test]
    async fn inserts_price_row() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let ts = utc(2025, 1, 1, 12, 0, 0);
      repo.insert_price(34, 5.5, ts).await.unwrap();

      let result = repo.latest_price(34).await.unwrap();
      assert!(result.is_some());
    }
  }

  async fn insert_character_and_asset(db: &DatabaseConnection, char_id: i64, type_id: i32, qty: i32) {
    use sea_orm::ActiveValue::Set;
    crate::entities::character::Entity::insert(crate::entities::character::ActiveModel {
      access_token: Set(String::new()),
      charisma: Set(None),
      corp_id: Set(0),
      corp_name: Set(String::new()),
      granted_scopes: Set(None),
      id: Set(char_id),
      intelligence: Set(None),
      isk_balance: Set(None),
      location_docked: Set(None),
      location_name: Set(None),
      memory: Set(None),
      name: Set(format!("Char {char_id}")),
      perception: Set(None),
      portrait_tone: Set(0),
      refresh_token: Set(String::new()),
      sort_order: Set(0),
      token_expires_at: Set(0),
      willpower: Set(None),
    })
    .exec(db)
    .await
    .unwrap();

    use crate::entities::character_asset::{ActiveModel as AssetActive, Entity as AssetEntity};
    AssetEntity::insert(AssetActive {
      character_id: Set(char_id),
      is_blueprint_copy: Set(None),
      is_singleton: Set(false),
      item_id: Set(type_id as i64 * 1000 + char_id),
      location_flag: Set("Hangar".to_string()),
      location_id: Set(60003760),
      location_type: Set("station".to_string()),
      quantity: Set(qty),
      type_id: Set(type_id),
    })
    .exec(db)
    .await
    .unwrap();
  }
}
