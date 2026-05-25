//! Database entity for EVE Online item types.

use pod_model::{DogmaAttributeEntry, DogmaEffectEntry, ItemType};
use sea_orm::{Set, prelude::*};

use crate::entities::{dogma_attribute, dogma_effect};

/// A specific item type (ship, module, commodity, etc.) in the EVE Online universe.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "item_types")]
pub struct Model {
  /// Maximum cargo or fuel capacity in cubic metres, if applicable.
  pub capacity: Option<f64>,
  /// Human-readable description of the item type.
  pub description: String,
  /// Dogma attribute overrides applied to this item type.
  pub dogma_attributes: dogma_attribute::List,
  /// Dogma effects active on this item type.
  pub dogma_effects: dogma_effect::List,
  /// Identifier of the associated 3-D graphic resource, if any.
  pub graphic_id: Option<i32>,
  /// Identifier of the UI icon used for this item type, if any.
  pub icon_id: Option<i32>,
  /// Unique item type identifier, sourced from the ESI.
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// Whether this item type is an abyssal (mutated) module type.
  pub is_abyssal: bool,
  /// The item group this type belongs to.
  #[sea_orm(belongs_to, from = "item_group_id", to = "id")]
  pub item_group: HasOne<super::item_group::Entity>,
  /// Foreign key referencing the parent item group.
  pub item_group_id: i32,
  /// The market group this type is listed under, if any.
  #[sea_orm(belongs_to, from = "market_group_id", to = "id")]
  pub market_group: HasOne<super::market_group::Entity>,
  /// Foreign key referencing the parent market group, if any.
  pub market_group_id: Option<i32>,
  /// Mass of the item in kilograms, if applicable.
  pub mass: Option<f64>,
  /// Display name of the item type.
  pub name: String,
  /// Volume of the item when packaged, in cubic metres.
  pub packaged_volume: Option<f64>,
  /// Number of units produced or consumed per batch action.
  pub portion_size: Option<i32>,
  /// Whether this item type is visible in the public game client.
  pub published: bool,
  /// Radius of the item's bounding sphere in metres, if applicable.
  pub radius: Option<f64>,
  /// Volume of the item in cubic metres, if applicable.
  pub volume: Option<f64>,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<dogma_attribute::Entry> for DogmaAttributeEntry {
  fn from(entry: dogma_attribute::Entry) -> Self {
    Self::new(entry.attribute_id, entry.value)
  }
}

impl From<DogmaAttributeEntry> for dogma_attribute::Entry {
  fn from(attr: DogmaAttributeEntry) -> Self {
    Self {
      attribute_id: *attr.attribute_id(),
      value: *attr.value(),
    }
  }
}

impl From<dogma_effect::Entry> for DogmaEffectEntry {
  fn from(entry: dogma_effect::Entry) -> Self {
    Self::new(entry.effect_id, entry.is_default)
  }
}

impl From<DogmaEffectEntry> for dogma_effect::Entry {
  fn from(effect: DogmaEffectEntry) -> Self {
    Self {
      effect_id: *effect.effect_id(),
      is_default: *effect.is_default(),
    }
  }
}

impl From<Model> for ItemType {
  fn from(entity: Model) -> Self {
    let mut model = ItemType::new(entity.id, entity.name);
    *model.dogma_attributes_mut() = entity.dogma_attributes.0.into_iter().map(Into::into).collect();
    *model.dogma_effects_mut() = entity.dogma_effects.0.into_iter().map(Into::into).collect();
    model
      .set_capacity(entity.capacity)
      .set_description(entity.description)
      .set_graphic_id(entity.graphic_id)
      .set_icon_id(entity.icon_id)
      .set_is_abyssal(entity.is_abyssal)
      .set_item_group_id(entity.item_group_id)
      .set_market_group_id(entity.market_group_id)
      .set_mass(entity.mass)
      .set_packaged_volume(entity.packaged_volume)
      .set_portion_size(entity.portion_size)
      .set_published(entity.published)
      .set_radius(entity.radius)
      .set_volume(entity.volume)
      .mark_persisted();
    model
  }
}

impl From<ModelEx> for ItemType {
  fn from(entity: ModelEx) -> Self {
    let mut model = ItemType::new(entity.id, entity.name);
    *model.dogma_attributes_mut() = entity.dogma_attributes.0.into_iter().map(Into::into).collect();
    *model.dogma_effects_mut() = entity.dogma_effects.0.into_iter().map(Into::into).collect();
    model
      .set_capacity(entity.capacity)
      .set_description(entity.description)
      .set_graphic_id(entity.graphic_id)
      .set_icon_id(entity.icon_id)
      .set_is_abyssal(entity.is_abyssal)
      .set_item_group(entity.item_group.into_option().map(Into::into))
      .set_item_group_id(entity.item_group_id)
      .set_market_group(entity.market_group.into_option().map(Into::into))
      .set_market_group_id(entity.market_group_id)
      .set_mass(entity.mass)
      .set_packaged_volume(entity.packaged_volume)
      .set_portion_size(entity.portion_size)
      .set_published(entity.published)
      .set_radius(entity.radius)
      .set_volume(entity.volume)
      .mark_persisted();
    model
  }
}

impl From<ItemType> for ActiveModel {
  fn from(model: ItemType) -> Self {
    Self {
      capacity: Set(*model.capacity()),
      description: Set(model.description().clone()),
      dogma_attributes: Set(dogma_attribute::List(
        model.dogma_attributes().iter().cloned().map(Into::into).collect(),
      )),
      dogma_effects: Set(dogma_effect::List(
        model.dogma_effects().iter().cloned().map(Into::into).collect(),
      )),
      graphic_id: Set(*model.graphic_id()),
      id: Set(*model.id()),
      icon_id: Set(*model.icon_id()),
      is_abyssal: Set(*model.is_abyssal()),
      item_group_id: Set(*model.item_group_id()),
      market_group_id: Set(*model.market_group_id()),
      mass: Set(*model.mass()),
      name: Set(model.name().clone()),
      packaged_volume: Set(*model.packaged_volume()),
      portion_size: Set(*model.portion_size()),
      published: Set(*model.published()),
      radius: Set(*model.radius()),
      volume: Set(*model.volume()),
    }
  }
}

impl From<ItemType> for ActiveModelEx {
  fn from(model: ItemType) -> Self {
    Self {
      capacity: Set(*model.capacity()),
      description: Set(model.description().clone()),
      dogma_attributes: Set(dogma_attribute::List(
        model.dogma_attributes().iter().cloned().map(Into::into).collect(),
      )),
      dogma_effects: Set(dogma_effect::List(
        model.dogma_effects().iter().cloned().map(Into::into).collect(),
      )),
      graphic_id: Set(*model.graphic_id()),
      id: Set(*model.id()),
      icon_id: Set(*model.icon_id()),
      is_abyssal: Set(*model.is_abyssal()),
      item_group: Default::default(),
      item_group_id: Set(*model.item_group_id()),
      market_group: Default::default(),
      market_group_id: Set(*model.market_group_id()),
      mass: Set(*model.mass()),
      name: Set(model.name().clone()),
      packaged_volume: Set(*model.packaged_volume()),
      portion_size: Set(*model.portion_size()),
      published: Set(*model.published()),
      radius: Set(*model.radius()),
      volume: Set(*model.volume()),
    }
  }
}
