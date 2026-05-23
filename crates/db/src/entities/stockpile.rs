//! Database entity for stockpiles.

use pod_model::Stockpile;
use sea_orm::{Set, prelude::*};

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
  /// Item requirements belonging to this stockpile.
  #[sea_orm(has_many)]
  pub items: HasMany<super::stockpile_item::Entity>,
  /// Optional location this stockpile is scoped to; None means all locations.
  pub location_id: Option<i64>,
  /// Display name of the stockpile.
  pub name: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for Stockpile {
  fn from(entity: Model) -> Self {
    Stockpile::new(entity.id, entity.name)
  }
}

impl From<ModelEx> for Stockpile {
  fn from(entity: ModelEx) -> Self {
    let mut model = Stockpile::new(entity.id, entity.name);
    *model.items_mut() = entity.items.into_iter().map(Into::into).collect();
    model
  }
}

impl From<Stockpile> for ActiveModel {
  fn from(model: Stockpile) -> Self {
    Self {
      character_id: Set(*model.character_id()),
      id: Set(*model.id()),
      location_id: Set(*model.location_id()),
      name: Set(model.name().clone()),
    }
  }
}

impl From<Stockpile> for ActiveModelEx {
  fn from(model: Stockpile) -> Self {
    Self {
      character_id: Set(*model.character_id()),
      id: Set(*model.id()),
      items: model.items().iter().cloned().map(Into::into).collect::<Vec<_>>().into(),
      location_id: Set(*model.location_id()),
      name: Set(model.name().clone()),
    }
  }
}
