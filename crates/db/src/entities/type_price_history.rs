//! Database entity for daily OHLC price aggregates per EVE type.

use sea_orm::prelude::*;

/// A daily OHLC aggregate stored in the `type_price_histories` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "type_price_histories")]
pub struct Model {
  /// Mean price across all samples for the day.
  pub avg: f64,
  /// UTC calendar date for this aggregate (ISO-8601 date string).
  pub date: String,
  /// Closing price (last sample of the day).
  pub close: f64,
  /// Highest price observed during the day.
  pub high: f64,
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i64,
  /// Lowest price observed during the day.
  pub low: f64,
  /// Opening price (first sample of the day).
  pub open: f64,
  /// Number of intraday observations used to build this aggregate.
  pub sample_count: i32,
  /// EVE type ID.
  pub type_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
