//! Database entity for stockpile items.

use pod_model::StockpileItem;
use sea_orm::{Set, prelude::*};

/// An item requirement record stored in the `stockpile_items` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "stockpile_items")]
pub struct Model {
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i64,
  /// The stockpile this item belongs to.
  #[sea_orm(belongs_to, from = "stockpile_id", to = "id")]
  pub stockpile: HasOne<super::stockpile::Entity>,
  /// FK to the owning stockpile in `stockpiles`.
  pub stockpile_id: i64,
  /// Desired quantity to keep stocked.
  pub target_quantity: i32,
  /// EVE type ID of the item.
  pub type_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for StockpileItem {
  fn from(entity: Model) -> Self {
    StockpileItem::new(entity.id, entity.stockpile_id, entity.type_id, entity.target_quantity)
  }
}

impl From<ModelEx> for StockpileItem {
  fn from(entity: ModelEx) -> Self {
    StockpileItem::new(entity.id, entity.stockpile_id, entity.type_id, entity.target_quantity)
  }
}

impl From<StockpileItem> for ActiveModel {
  fn from(model: StockpileItem) -> Self {
    Self {
      id: Set(*model.id()),
      stockpile_id: Set(*model.stockpile_id()),
      target_quantity: Set(*model.target_quantity()),
      type_id: Set(*model.type_id()),
    }
  }
}

impl From<StockpileItem> for ActiveModelEx {
  fn from(model: StockpileItem) -> Self {
    Self {
      id: Set(*model.id()),
      stockpile: Default::default(),
      stockpile_id: Set(*model.stockpile_id()),
      target_quantity: Set(*model.target_quantity()),
      type_id: Set(*model.type_id()),
    }
  }
}
