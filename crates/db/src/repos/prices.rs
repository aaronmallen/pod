//! Repository for intraday price observations and daily OHLC aggregates.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, NaiveDate, Utc};
use sea_orm::{
  ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, Order, QueryFilter, QueryOrder, sea_query::OnConflict,
};

use crate::{
  Error,
  entities::{
    character_asset::{Column as AssetColumn, Entity as AssetEntity},
    type_price::{ActiveModel as PriceActive, Column as PriceColumn, Entity as PriceEntity},
    type_price_history::{ActiveModel as HistoryActive, Column as HistoryColumn, Entity as HistoryEntity},
  },
};

fn accumulate_nav_by_date(
  histories: &[crate::entities::type_price_history::Model],
  qty_map: &HashMap<i32, i64>,
) -> HashMap<NaiveDate, f64> {
  let mut nav_by_date = HashMap::new();
  for h in histories {
    if let (Ok(date), Some(&qty)) = (NaiveDate::parse_from_str(&h.date, "%Y-%m-%d"), qty_map.get(&h.type_id)) {
      *nav_by_date.entry(date).or_insert(0.0) += h.close * qty as f64;
    }
  }
  nav_by_date
}

fn compute_today_nav(latest_prices: &HashMap<i32, f64>, qty_map: &HashMap<i32, i64>) -> f64 {
  latest_prices
    .iter()
    .filter_map(|(tid, price)| qty_map.get(tid).map(|&qty| price * qty as f64))
    .sum()
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
  pub async fn insert_price(
    &self,
    type_id: i32,
    price: f64,
    adjusted_price: Option<f64>,
    fetched_at: DateTime<Utc>,
  ) -> Result<(), Error> {
    let active = PriceActive {
      adjusted_price: ActiveValue::Set(adjusted_price),
      fetched_at: ActiveValue::Set(fetched_at.to_rfc3339()),
      id: ActiveValue::NotSet,
      price: ActiveValue::Set(price),
      type_id: ActiveValue::Set(type_id),
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

  /// Returns the most recent price for each `type_id` in the input slice.
  ///
  /// Queries `type_prices` (intraday) for all IDs in one call, then falls back
  /// to `type_price_histories.close` for any IDs not found intraday. Returns
  /// an empty map when `type_ids` is empty.
  pub async fn latest_prices(&self, type_ids: &[i32]) -> Result<HashMap<i32, f64>, Error> {
    if type_ids.is_empty() {
      return Ok(HashMap::new());
    }

    let intraday = PriceEntity::find()
      .filter(PriceColumn::TypeId.is_in(type_ids.to_vec()))
      .order_by(PriceColumn::FetchedAt, Order::Desc)
      .all(self.db)
      .await?;

    let mut result: HashMap<i32, f64> = HashMap::new();
    for row in intraday {
      result.entry(row.type_id).or_insert(row.price);
    }

    let missing: Vec<i32> = type_ids.iter().copied().filter(|id| !result.contains_key(id)).collect();

    if !missing.is_empty() {
      let history = HistoryEntity::find()
        .filter(HistoryColumn::TypeId.is_in(missing))
        .order_by(HistoryColumn::Date, Order::Desc)
        .all(self.db)
        .await?;

      for row in history {
        result.entry(row.type_id).or_insert(row.close);
      }
    }

    Ok(result)
  }

  /// Returns all distinct type IDs that should have prices tracked.
  ///
  /// The set is the UNION of type IDs present in `character_assets` and
  /// `type_price_histories`.
  pub async fn types_to_track(&self) -> Result<Vec<i32>, Error> {
    let asset_ids: HashSet<i32> = AssetEntity::find()
      .all(self.db)
      .await?
      .into_iter()
      .map(|a| a.type_id)
      .collect();

    let history_ids: HashSet<i32> = HistoryEntity::find()
      .all(self.db)
      .await?
      .into_iter()
      .map(|h| h.type_id)
      .collect();

    Ok(asset_ids.union(&history_ids).copied().collect())
  }

  /// Aggregates all intraday `type_prices` rows for `date` into OHLC history
  /// records and deletes the processed rows.
  ///
  /// For each type that has rows on `date`, computes open (first fetched),
  /// high (max), low (min), close (last fetched), avg (mean), and
  /// sample_count, then upserts into `type_price_histories`. Processed
  /// intraday rows are deleted after all upserts complete.
  pub async fn aggregate_and_prune(&self, date: NaiveDate) -> Result<(), Error> {
    let date_str = date.format("%Y-%m-%d").to_string();
    let next_str = (date + Duration::days(1)).format("%Y-%m-%d").to_string();

    let rows = PriceEntity::find()
      .filter(PriceColumn::FetchedAt.gte(&date_str))
      .filter(PriceColumn::FetchedAt.lt(&next_str))
      .order_by(PriceColumn::FetchedAt, Order::Asc)
      .all(self.db)
      .await?;

    if rows.is_empty() {
      return Ok(());
    }

    let mut by_type: HashMap<i32, Vec<f64>> = HashMap::new();
    for row in &rows {
      by_type.entry(row.type_id).or_default().push(row.price);
    }

    for (tid, samples) in by_type {
      let open = *samples.first().unwrap_or(&0.0);
      let close = *samples.last().unwrap_or(&0.0);
      let high = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
      let low = samples.iter().cloned().fold(f64::INFINITY, f64::min);
      let avg = samples.iter().sum::<f64>() / samples.len() as f64;
      let sample_count = samples.len() as i32;

      let active = HistoryActive {
        id: ActiveValue::NotSet,
        type_id: ActiveValue::Set(tid),
        date: ActiveValue::Set(date_str.clone()),
        open: ActiveValue::Set(open),
        high: ActiveValue::Set(high),
        low: ActiveValue::Set(low),
        close: ActiveValue::Set(close),
        avg: ActiveValue::Set(avg),
        sample_count: ActiveValue::Set(sample_count),
      };

      HistoryEntity::insert(active)
        .on_conflict(
          OnConflict::columns([HistoryColumn::TypeId, HistoryColumn::Date])
            .update_columns([
              HistoryColumn::Open,
              HistoryColumn::High,
              HistoryColumn::Low,
              HistoryColumn::Close,
              HistoryColumn::Avg,
              HistoryColumn::SampleCount,
            ])
            .to_owned(),
        )
        .exec(self.db)
        .await?;
    }

    PriceEntity::delete_many()
      .filter(PriceColumn::FetchedAt.gte(&date_str))
      .filter(PriceColumn::FetchedAt.lt(&next_str))
      .exec(self.db)
      .await?;

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

    let mut nav_by_date = accumulate_nav_by_date(&histories, &qty_map);

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

    let today_nav = compute_today_nav(&latest_prices, &qty_map);

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

    let rows = PriceEntity::find()
      .filter(PriceColumn::FetchedAt.lt(&before_str))
      .all(self.db)
      .await?;

    let mut dates: HashSet<NaiveDate> = HashSet::new();
    for row in rows {
      if let Some(d) = row
        .fetched_at
        .get(..10)
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
      {
        dates.insert(d);
      }
    }

    let mut sorted: Vec<NaiveDate> = dates.into_iter().collect();
    sorted.sort();
    Ok(sorted)
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
      repo.insert_price(34, 5.5, None, ts).await.unwrap();

      let result = repo.latest_price(34).await.unwrap();
      assert_eq!(result, Some(5.5));
    }

    #[tokio::test]
    async fn returns_most_recent_intraday_when_multiple_exist() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo
        .insert_price(34, 5.5, None, utc(2025, 1, 1, 10, 0, 0))
        .await
        .unwrap();
      repo
        .insert_price(34, 6.0, None, utc(2025, 1, 1, 12, 0, 0))
        .await
        .unwrap();
      repo
        .insert_price(34, 5.8, None, utc(2025, 1, 1, 11, 0, 0))
        .await
        .unwrap();

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

  mod latest_prices {
    use pretty_assertions::assert_eq;

    use super::*;

    async fn insert_history(db: &DatabaseConnection, type_id: i32, date: &str, close: f64) {
      use sea_orm::ActiveValue::{NotSet, Set};

      use crate::entities::type_price_history::{ActiveModel as HistActive, Entity as HistEntity};
      HistEntity::insert(HistActive {
        id: NotSet,
        type_id: Set(type_id),
        date: Set(date.to_string()),
        open: Set(close),
        high: Set(close),
        low: Set(close),
        close: Set(close),
        avg: Set(close),
        sample_count: Set(1),
      })
      .exec(db)
      .await
      .unwrap();
    }

    #[tokio::test]
    async fn it_returns_empty_map_for_empty_input() {
      let db = setup_db().await;
      let repo = Repo::new(&db);

      let result = repo.latest_prices(&[]).await.unwrap();

      assert!(result.is_empty());
    }

    #[tokio::test]
    async fn it_returns_intraday_price_when_present() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo
        .insert_price(34, 5.5, None, utc(2025, 1, 1, 12, 0, 0))
        .await
        .unwrap();

      let result = repo.latest_prices(&[34]).await.unwrap();

      assert_eq!(result.get(&34), Some(&5.5));
    }

    #[tokio::test]
    async fn it_falls_back_to_history_when_no_intraday() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      insert_history(&db, 34, "2025-01-01", 5.9).await;

      let result = repo.latest_prices(&[34]).await.unwrap();

      assert_eq!(result.get(&34), Some(&5.9));
    }

    #[tokio::test]
    async fn it_prefers_intraday_over_history() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo
        .insert_price(34, 7.0, None, utc(2025, 1, 1, 12, 0, 0))
        .await
        .unwrap();
      insert_history(&db, 34, "2025-01-01", 5.9).await;

      let result = repo.latest_prices(&[34]).await.unwrap();

      assert_eq!(result.get(&34), Some(&7.0));
    }

    #[tokio::test]
    async fn it_resolves_multiple_type_ids_in_one_call() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      repo
        .insert_price(34, 5.5, None, utc(2025, 1, 1, 12, 0, 0))
        .await
        .unwrap();
      insert_history(&db, 35, "2025-01-01", 3.0).await;

      let result = repo.latest_prices(&[34, 35]).await.unwrap();

      assert_eq!(result.get(&34), Some(&5.5));
      assert_eq!(result.get(&35), Some(&3.0));
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

      repo
        .insert_price(34, 5.0, None, utc(2025, 1, 1, 12, 0, 0))
        .await
        .unwrap();
      repo
        .insert_price(34, 5.5, None, utc(2025, 1, 3, 12, 0, 0))
        .await
        .unwrap();

      let before = NaiveDate::from_ymd_opt(2025, 1, 3).unwrap();
      let result = repo.dates_needing_aggregation(before).await.unwrap();
      assert_eq!(result.len(), 1);
      assert_eq!(result[0], NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
    }
  }

  mod insert_price {
    use sea_orm::EntityTrait;

    use super::*;
    use crate::entities::type_price::{Column as PriceColumn, Entity as PriceEntity};

    #[tokio::test]
    async fn it_inserts_price_row() {
      let db = setup_db().await;
      let repo = Repo::new(&db);
      let ts = utc(2025, 1, 1, 12, 0, 0);
      repo.insert_price(34, 5.5, None, ts).await.unwrap();

      let result = repo.latest_price(34).await.unwrap();
      assert!(result.is_some());
    }

    #[tokio::test]
    async fn it_persists_adjusted_price_when_provided() {
      use sea_orm::{ColumnTrait, QueryFilter};

      let db = setup_db().await;
      let repo = Repo::new(&db);
      let ts = utc(2025, 1, 1, 12, 0, 0);
      repo.insert_price(34, 5.5, Some(4.8), ts).await.unwrap();

      let row = PriceEntity::find()
        .filter(PriceColumn::TypeId.eq(34))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
      assert_eq!(row.adjusted_price, Some(4.8));
    }

    #[tokio::test]
    async fn it_stores_null_adjusted_price_when_not_provided() {
      use sea_orm::{ColumnTrait, QueryFilter};

      let db = setup_db().await;
      let repo = Repo::new(&db);
      let ts = utc(2025, 1, 1, 12, 0, 0);
      repo.insert_price(34, 5.5, None, ts).await.unwrap();

      let row = PriceEntity::find()
        .filter(PriceColumn::TypeId.eq(34))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
      assert_eq!(row.adjusted_price, None);
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
      is_active_ship: Set(false),
      is_blueprint_copy: Set(None),
      is_singleton: Set(false),
      item_id: Set(type_id as i64 * 1000 + char_id),
      location_flag: Set("Hangar".to_string()),
      location_id: Set(60003760),
      location_type: Set("station".to_string()),
      quantity: Set(qty),
      ship_name: Set(None),
      type_id: Set(type_id),
    })
    .exec(db)
    .await
    .unwrap();
  }
}
