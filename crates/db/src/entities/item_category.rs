//! Database entity for EVE Online item categories.

use pod_model::ItemCategory;
use sea_orm::{Set, prelude::*};

/// A top-level grouping of item groups in the EVE Online universe.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "item_categories")]
pub struct Model {
  /// Unique category identifier, sourced from the ESI.
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// Item groups belonging to this category.
  #[sea_orm(has_many)]
  pub item_groups: HasMany<super::item_group::Entity>,
  /// Display name of the category.
  pub name: String,
  /// Whether the category is visible in the public game client.
  pub published: bool,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for ItemCategory {
  fn from(entity: Model) -> Self {
    let mut model = ItemCategory::new(entity.id, entity.name);
    if !entity.published {
      model.unpublish();
    }
    model.mark_persisted();
    model
  }
}

impl From<ModelEx> for ItemCategory {
  fn from(entity: ModelEx) -> Self {
    let mut model = ItemCategory::new(entity.id, entity.name);
    *model.item_groups_mut() = entity.item_groups.into_iter().map(Into::into).collect();
    if !entity.published {
      model.unpublish();
    }
    model.mark_persisted();
    model
  }
}

impl From<ItemCategory> for ActiveModel {
  fn from(model: ItemCategory) -> Self {
    Self {
      id: Set(*model.id()),
      name: Set(model.name().clone()),
      published: Set(model.is_published()),
    }
  }
}

impl From<ItemCategory> for ActiveModelEx {
  fn from(model: ItemCategory) -> Self {
    Self {
      id: Set(*model.id()),
      item_groups: model
        .item_groups()
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
