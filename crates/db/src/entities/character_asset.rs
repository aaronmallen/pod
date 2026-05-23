//! Database entity for EVE Online character assets.

use pod_model::CharacterAsset;
use sea_orm::prelude::*;

/// An asset row stored in the `character_assets` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "character_assets")]
pub struct Model {
  /// The character that owns this asset.
  #[sea_orm(belongs_to, from = "character_id", to = "id")]
  pub character: HasOne<super::character::Entity>,
  /// ID of the owning character.
  pub character_id: i64,
  /// True when this row is a synthetic active-ship entry injected by the sync job.
  pub is_active_ship: bool,
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
  /// Display name of the ship when `is_active_ship` is true; `None` otherwise.
  pub ship_name: Option<String>,
  /// EVE type ID of the item.
  pub type_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for CharacterAsset {
  fn from(m: Model) -> Self {
    Self {
      character_id: m.character_id,
      is_active_ship: m.is_active_ship,
      is_blueprint_copy: m.is_blueprint_copy,
      is_singleton: m.is_singleton,
      item_id: m.item_id,
      location_flag: m.location_flag,
      location_id: m.location_id,
      location_type: m.location_type,
      quantity: m.quantity,
      ship_name: m.ship_name,
      type_id: m.type_id,
    }
  }
}

impl From<ModelEx> for CharacterAsset {
  fn from(m: ModelEx) -> Self {
    Self {
      character_id: m.character_id,
      is_active_ship: m.is_active_ship,
      is_blueprint_copy: m.is_blueprint_copy,
      is_singleton: m.is_singleton,
      item_id: m.item_id,
      location_flag: m.location_flag,
      location_id: m.location_id,
      location_type: m.location_type,
      quantity: m.quantity,
      ship_name: m.ship_name,
      type_id: m.type_id,
    }
  }
}
