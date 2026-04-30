//! Database entity for EVE Online character assets.

use pod_model::CharacterAsset;
use sea_orm::prelude::*;

/// An asset row stored in the `character_assets` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "character_assets")]
pub struct Model {
  /// ID of the owning character.
  pub character_id: i64,
  /// True if this is a blueprint copy; false if original; None if not a blueprint.
  pub is_blueprint_copy: Option<bool>,
  /// Whether this item is a packaged singleton.
  pub is_singleton: bool,
  /// Unique EVE item instance ID (primary key).
  #[sea_orm(primary_key, auto_increment = false)]
  pub item_id: i64,
  /// Asset container flag (e.g. "Cargo", "HiSlot0").
  pub location_flag: String,
  /// ID of the location where the asset resides.
  pub location_id: i64,
  /// Location category ("station", "solar_system", etc.).
  pub location_type: String,
  /// Stack size.
  pub quantity: i32,
  /// EVE type ID of the item.
  pub type_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for CharacterAsset {
  fn from(m: Model) -> Self {
    Self {
      item_id: m.item_id,
      character_id: m.character_id,
      type_id: m.type_id,
      location_id: m.location_id,
      location_type: m.location_type,
      location_flag: m.location_flag,
      quantity: m.quantity,
      is_singleton: m.is_singleton,
      is_blueprint_copy: m.is_blueprint_copy,
    }
  }
}
