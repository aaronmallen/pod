//! Database entity for EVE Online character clones.

use sea_orm::prelude::*;

/// A clone record stored in the `character_clones` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "character_clones")]
pub struct Model {
  /// ID of the owning character.
  pub character_id: i64,
  /// Primary key: EVE clone identifier (0 for the active implant set).
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i64,
  /// ISO-8601 timestamp when the clone was installed, if known.
  pub installed_at: Option<String>,
  /// Whether this is the character's active clone.
  pub is_active: bool,
  /// Location structure or station ID.
  pub location_id: i64,
  /// Optional user-assigned name for jump clones.
  pub name: Option<String>,
  /// Resolved region name for display.
  pub region_name: String,
  /// Resolved station or structure name for display.
  pub station_name: String,
  /// ISO-8601 timestamp when this record was last synced from ESI.
  pub synced_at: String,
  /// Solar system ID for this clone's location.
  pub system_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
