//! Database entity for abyssal module stat bounds.

use pod_model::AbyssalModuleStat;
use sea_orm::{Set, prelude::*};

/// A single attribute min/max multiplier range for an abyssal module type.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "abyssal_module_stats")]
pub struct Model {
  /// The resulting abyssal type ID (the mutated item type).
  pub abyssal_type_id: i32,
  /// The dogma attribute ID this bound applies to.
  pub attribute_id: i32,
  /// Auto-incremented primary key.
  #[sea_orm(primary_key)]
  pub id: i32,
  /// Maximum multiplier that can be applied to the base stat.
  pub max_mult: f64,
  /// Minimum multiplier that can be applied to the base stat.
  pub min_mult: f64,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for AbyssalModuleStat {
  fn from(entity: Model) -> Self {
    AbyssalModuleStat::new(
      entity.abyssal_type_id,
      entity.attribute_id,
      entity.min_mult,
      entity.max_mult,
    )
  }
}

impl From<AbyssalModuleStat> for ActiveModel {
  fn from(model: AbyssalModuleStat) -> Self {
    Self {
      abyssal_type_id: Set(*model.abyssal_type_id()),
      attribute_id: Set(*model.attribute_id()),
      id: Default::default(),
      max_mult: Set(*model.max_mult()),
      min_mult: Set(*model.min_mult()),
    }
  }
}
