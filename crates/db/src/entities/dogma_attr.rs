//! Database entity for the dogma_attributes table.

use pod_model::DogmaAttr;
use sea_orm::{Set, prelude::*};

/// A dogma attribute definition row in the `dogma_attributes` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "dogma_attributes")]
pub struct Model {
  /// The EVE dogma attribute identifier.
  #[sea_orm(unique)]
  pub attribute_id: i32,
  /// Default value for this attribute when not overridden on an item.
  pub default_value: Option<f64>,
  /// Long-form description.
  pub description: Option<String>,
  /// Localized English display name.
  pub display_name: Option<String>,
  /// Whether a higher value is generally better.
  pub high_is_good: bool,
  /// EVE icon ID for the attribute.
  pub icon_id: Option<i32>,
  /// Auto-incremented primary key.
  #[sea_orm(primary_key)]
  pub id: i32,
  /// Internal non-localized attribute name.
  pub name: String,
  /// Whether visible in the public game client.
  pub published: bool,
  /// Whether this attribute is affected by stacking penalties.
  pub stackable: bool,
  /// EVE unit ID for value formatting.
  pub unit_id: Option<i32>,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for DogmaAttr {
  fn from(entity: Model) -> Self {
    let mut m = DogmaAttr::new(entity.attribute_id, entity.name);
    m.set_default_value(entity.default_value)
      .set_description(entity.description)
      .set_display_name(entity.display_name)
      .set_high_is_good(entity.high_is_good)
      .set_icon_id(entity.icon_id)
      .set_published(entity.published)
      .set_stackable(entity.stackable)
      .set_unit_id(entity.unit_id);
    m
  }
}

impl From<DogmaAttr> for ActiveModel {
  fn from(model: DogmaAttr) -> Self {
    Self {
      attribute_id: Set(*model.attribute_id()),
      default_value: Set(*model.default_value()),
      description: Set(model.description().clone()),
      display_name: Set(model.display_name().clone()),
      high_is_good: Set(*model.high_is_good()),
      icon_id: Set(*model.icon_id()),
      id: Default::default(),
      name: Set(model.name().clone()),
      published: Set(*model.published()),
      stackable: Set(*model.stackable()),
      unit_id: Set(*model.unit_id()),
    }
  }
}
