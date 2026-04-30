//! Database entity for stockpile items.

use sea_orm::prelude::*;

/// An item requirement record stored in the `stockpile_items` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "stockpile_items")]
pub struct Model {
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i64,
  /// FK to the owning stockpile in `stockpiles`.
  pub stockpile_id: i64,
  /// Desired quantity to keep stocked.
  pub target_quantity: i32,
  /// EVE type ID of the item.
  pub type_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
