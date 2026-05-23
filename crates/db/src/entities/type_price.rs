//! Database entity for intraday EVE type price observations.

use sea_orm::prelude::*;

/// An intraday price observation stored in the `type_prices` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "type_prices")]
pub struct Model {
  /// CCP's adjusted price for this type from `/v1/markets/prices/`.
  pub adjusted_price: Option<f64>,
  /// ISO-8601 timestamp when the price was fetched.
  pub fetched_at: String,
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i64,
  /// Lowest Jita sell order price at fetch time.
  pub price: f64,
  /// EVE type ID.
  pub type_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
