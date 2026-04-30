//! Database entity for EVE Online character standings.

use sea_orm::prelude::*;

/// A standing record stored in the `character_standings` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "character_standings")]
pub struct Model {
  /// ID of the owning character.
  pub character_id: i64,
  /// Standing modified by skill bonuses.
  pub effective_standing: f64,
  /// ID of the entity toward which the standing applies.
  pub from_id: i32,
  /// Resolved display name of the entity.
  pub from_name: String,
  /// Entity type: faction, corp, or agent.
  pub from_type: String,
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i32,
  /// Raw standing value as returned by ESI.
  pub raw_standing: f64,
  /// ISO-8601 timestamp when this record was last synced from ESI.
  pub synced_at: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
