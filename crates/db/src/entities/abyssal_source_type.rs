//! Database entity for abyssal source module type IDs.

use sea_orm::{Set, prelude::*};

/// A source module type ID that can be mutated by a mutaplasmid.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "abyssal_source_types")]
pub struct Model {
  /// The source module type ID (the base module that can be mutated).
  #[sea_orm(primary_key, auto_increment = false)]
  pub source_type_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<i32> for ActiveModel {
  fn from(id: i32) -> Self {
    Self {
      source_type_id: Set(id),
    }
  }
}
