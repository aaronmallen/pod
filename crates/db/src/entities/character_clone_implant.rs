//! Database entity for implants installed in a character clone.

use sea_orm::prelude::*;

/// An implant slot record stored in the `character_clone_implants` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "character_clone_implants")]
pub struct Model {
  /// Resolved attribute bonus description.
  pub attribute_bonus: String,
  /// FK to the owning clone in `character_clones`.
  pub clone_id: i64,
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i32,
  /// Resolved display name of the implant type.
  pub name: String,
  /// Implant slot number (1–10).
  pub slot: i32,
  /// EVE type ID of the implant.
  pub type_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
