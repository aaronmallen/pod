//! Database entity for wallet transactions.

use pod_model::WalletTransaction;
use sea_orm::prelude::*;
use validator::Validate;

/// A wallet transaction stored in the `wallet_transactions` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq, Validate)]
#[sea_orm(table_name = "wallet_transactions")]
pub struct Model {
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i32,
  /// ID of the owning character.
  pub character_id: i64,
  /// ESI transaction ID.
  pub transaction_id: i64,
  /// EVE item type ID.
  pub type_id: i32,
  /// Number of units in the transaction.
  pub quantity: i32,
  /// ISK per unit.
  pub unit_price: f64,
  /// Whether this was a buy order.
  pub is_buy: bool,
  /// ISO 8601 timestamp.
  #[validate(length(min = 1))]
  pub date: String,
  /// Station or structure ID.
  pub location_id: i64,
  /// ESI ID of the counterparty.
  pub client_id: i64,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for WalletTransaction {
  fn from(e: Model) -> Self {
    Self {
      character_id: e.character_id,
      transaction_id: e.transaction_id,
      type_id: e.type_id,
      quantity: e.quantity,
      unit_price: e.unit_price,
      is_buy: e.is_buy,
      date: e.date,
      location_id: e.location_id,
      client_id: e.client_id,
    }
  }
}
