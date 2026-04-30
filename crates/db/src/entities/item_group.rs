//! Database entity for EVE Online item groups.

use pod_model::ItemGroup;
use sea_orm::{Set, prelude::*};

/// A grouping of related item types within a single item category.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "item_groups")]
pub struct Model {
  /// Unique identifier for the item group.
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// The item category this group belongs to.
  #[sea_orm(belongs_to, from = "item_category_id", to = "id")]
  pub item_category: HasOne<super::item_category::Entity>,
  /// Foreign key referencing the parent item category.
  pub item_category_id: i32,
  /// Item types that belong to this group.
  #[sea_orm(has_many)]
  pub item_types: HasMany<super::item_type::Entity>,
  /// Display name of the item group.
  pub name: String,
  /// Whether this group is visible in the in-game market and UI.
  pub published: bool,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for ItemGroup {
  fn from(entity: Model) -> Self {
    let mut model = ItemGroup::new(entity.id, entity.item_category_id, entity.name);
    if !entity.published {
      model.unpublish();
    }
    model.mark_persisted();
    model
  }
}

impl From<ModelEx> for ItemGroup {
  fn from(entity: ModelEx) -> Self {
    let mut model = ItemGroup::new(entity.id, entity.item_category_id, entity.name);
    *model.item_types_mut() = entity.item_types.into_iter().map(Into::into).collect();
    if !entity.published {
      model.unpublish();
    }
    model.mark_persisted();
    model
  }
}

impl From<ItemGroup> for ActiveModel {
  fn from(model: ItemGroup) -> Self {
    Self {
      id: Set(*model.id()),
      item_category_id: Set(*model.item_category_id()),
      name: Set(model.name().clone()),
      published: Set(model.is_published()),
    }
  }
}

impl From<ItemGroup> for ActiveModelEx {
  fn from(model: ItemGroup) -> Self {
    Self {
      id: Set(*model.id()),
      item_category: Default::default(),
      item_category_id: Set(*model.item_category_id()),
      item_types: model
        .item_types()
        .iter()
        .cloned()
        .map(Into::into)
        .collect::<Vec<_>>()
        .into(),
      name: Set(model.name().clone()),
      published: Set(model.is_published()),
    }
  }
}
