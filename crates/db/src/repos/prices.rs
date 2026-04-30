//! Repository for intraday price observations and daily OHLC aggregates.

use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::{
  ActiveValue, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, FromQueryResult, Order,
  QueryFilter, QueryOrder, Statement,
};

use crate::{
  Error,
  entities::{
    type_price::{ActiveModel as PriceActive, Column as PriceColumn, Entity as PriceEntity},
    type_price_history::{ActiveModel as HistoryActive, Column as HistoryColumn, Entity as HistoryEntity},
  },
};

#[derive(Debug, FromQueryResult)]
struct TypeIdRow {
  type_id: i32,
}

#[derive(Debug, FromQueryResult)]
struct NavDayRow {
  date: String,
  nav: f64,
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

  /// Returns NAV history for the given character IDs going back `days` calendar days.
  ///
  /// For each date in `type_price_histories`, computes NAV =
  /// sum(close * quantity) across all `character_assets` rows whose
  /// `type_id` has a price entry on that date and whose `character_id` is
  /// in `char_ids`. Returns rows sorted by date ascending. Returns an empty
  /// vec if the result has fewer than 2 data points.
  pub async fn nav_history(&self, char_ids: &[i64], days: u32) -> Result<Vec<(NaiveDate, f64)>, Error> {
    if char_ids.is_empty() {
      return Ok(Vec::new());
    }

    let ids_csv = char_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
    let sql = format!(
      "
      SELECT h.date AS date, SUM(h.close * a.quantity) AS nav
      FROM type_price_histories h
      JOIN character_assets a ON a.type_id = h.type_id AND a.character_id IN ({ids_csv})
      WHERE h.date >= date('now', '-{days} days')
      GROUP BY h.date
      ORDER BY h.date ASC
      "
    );

    let rows = NavDayRow::find_by_statement(Statement::from_string(DbBackend::Sqlite, sql))
      .all(self.db)
      .await?;

    let mut result: Vec<(NaiveDate, f64)> = rows
      .into_iter()
      .filter_map(|r| NaiveDate::parse_from_str(&r.date, "%Y-%m-%d").ok().map(|d| (d, r.nav)))
      .collect();

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
