//! Database entity for abyssal source-type to attribute-ID mappings.

use sea_orm::{Set, prelude::*};

/// A row in `abyssal_source_attributes` linking a source module type to
/// a dogma attribute ID that can be mutated by a mutaplasmid.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "abyssal_source_attributes")]
pub struct Model {
  /// The dogma attribute ID applicable to this source type (composite PK).
  #[sea_orm(primary_key, auto_increment = false)]
  pub attr_id: i32,
  /// The base module type ID (composite primary key).
  #[sea_orm(primary_key, auto_increment = false)]
  pub source_type_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<(i32, i32)> for ActiveModel {
  fn from((source_type_id, attr_id): (i32, i32)) -> Self {
    Self {
      attr_id: Set(attr_id),
      source_type_id: Set(source_type_id),
    }
  }
}
