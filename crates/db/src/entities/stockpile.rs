//! Database entity for stockpiles.

use sea_orm::prelude::*;

/// A stockpile record stored in the `stockpiles` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "stockpiles")]
pub struct Model {
  /// Optional character this stockpile is scoped to; None means all characters.
  pub character_id: Option<i64>,
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i64,
  /// Optional location this stockpile is scoped to; None means all locations.
  pub location_id: Option<i64>,
  /// Display name of the stockpile.
  pub name: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
