//! Database entity for cached player-owned structure names.

use sea_orm::prelude::*;

/// A cached player structure name in the `structure_cache` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "structure_cache")]
pub struct Model {
  /// ESI structure ID (64-bit).
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i64,
  /// Display name of the structure.
  pub name: String,
  /// Solar system ID where this structure resides, if known.
  pub solar_system_id: Option<i64>,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
