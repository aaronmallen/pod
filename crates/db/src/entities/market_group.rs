//! SeaORM entity for the `market_groups` table.

use pod_model::MarketGroup;
use sea_orm::{Set, prelude::*};

/// Link definition that traverses the parent–child relationship in reverse,
/// yielding child market groups from a given parent.
pub struct ChildrenLink;

impl Linked for ChildrenLink {
  type FromEntity = Entity;
  type ToEntity = Entity;

  /// Returns the relation chain used to navigate from a parent group to its children.
  fn link(&self) -> Vec<RelationDef> {
    vec![Relation::ParentMarketGroup.def().rev()]
  }
}

/// A market group used to categorize item types in the EVE market browser.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "market_groups")]
pub struct Model {
  /// Human-readable description of the market group.
  pub description: Option<String>,
  /// Unique market group identifier.
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// Item types listed under this market group.
  #[sea_orm(has_many)]
  pub item_types: HasMany<super::item_type::Entity>,
  /// Display name of the market group.
  pub name: String,
  /// Optional parent group, forming a category hierarchy.
  #[sea_orm(
    self_ref,
    relation_enum = "ParentMarketGroup",
    from = "parent_market_group_id",
    to = "id"
  )]
  pub parent_market_group: HasOne<Entity>,
  /// Foreign key referencing the parent market group, if any.
  pub parent_market_group_id: Option<i32>,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for MarketGroup {
  fn from(entity: Model) -> Self {
    let mut model = MarketGroup::new(entity.id, entity.name);
    model
      .set_description(entity.description)
      .set_parent_market_group_id(entity.parent_market_group_id)
      .mark_persisted();
    model
  }
}

impl From<ModelEx> for MarketGroup {
  fn from(entity: ModelEx) -> Self {
    let mut model = MarketGroup::new(entity.id, entity.name);
    *model.item_types_mut() = entity.item_types.into_iter().map(Into::into).collect();
    model
      .set_description(entity.description)
      .set_parent_market_group_id(entity.parent_market_group_id)
      .mark_persisted();
    model
  }
}

impl From<MarketGroup> for ActiveModel {
  fn from(model: MarketGroup) -> Self {
    Self {
      description: Set(model.description().clone()),
      id: Set(*model.id()),
      name: Set(model.name().clone()),
      parent_market_group_id: Set(*model.parent_market_group_id()),
    }
  }
}

impl From<MarketGroup> for ActiveModelEx {
  fn from(model: MarketGroup) -> Self {
    Self {
      description: Set(model.description().clone()),
      id: Set(*model.id()),
      item_types: model
        .item_types()
        .iter()
        .cloned()
        .map(Into::into)
        .collect::<Vec<_>>()
        .into(),
      name: Set(model.name().clone()),
      parent_market_group: Default::default(),
      parent_market_group_id: Set(*model.parent_market_group_id()),
    }
  }
}
