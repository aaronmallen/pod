//! Database entity for EVE Online corporation assets.

use pod_model::CorporationAsset;
use sea_orm::prelude::*;

/// An asset row stored in the `corporation_assets` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "corporation_assets")]
pub struct Model {
  /// ID of the owning corporation.
  pub corporation_id: i64,
  /// True if this is a blueprint copy; false if original; None if not a blueprint.
  pub is_blueprint_copy: Option<bool>,
  /// Whether this item is a packaged singleton.
  pub is_singleton: bool,
  /// Unique EVE item instance ID (primary key).
  #[sea_orm(primary_key, auto_increment = false)]
  pub item_id: i64,
  /// Asset container flag (e.g. "CorpSAG1", "Cargo").
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

impl From<Model> for CorporationAsset {
  fn from(m: Model) -> Self {
    Self {
      corporation_id: m.corporation_id,
      is_blueprint_copy: m.is_blueprint_copy,
      is_singleton: m.is_singleton,
      item_id: m.item_id,
      location_flag: m.location_flag,
      location_id: m.location_id,
      location_type: m.location_type,
      quantity: m.quantity,
      type_id: m.type_id,
    }
  }
}
