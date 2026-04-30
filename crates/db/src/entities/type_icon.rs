//! Database entity for cached EVE type icon image data.

use sea_orm::prelude::*;

/// A cached 32-px icon for an EVE item type.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "type_icons")]
pub struct Model {
  /// Raw PNG bytes of the 32-px icon.
  pub data: Vec<u8>,
  /// EVE type ID (composite primary key part 1).
  #[sea_orm(primary_key, auto_increment = false)]
  pub type_id: i32,
  /// Icon variant, e.g. "icon" or "bp" (composite primary key part 2).
  #[sea_orm(primary_key, auto_increment = false)]
  pub variant: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
