//! Database entity for EVE Online character contact labels.

use sea_orm::prelude::*;

/// A contact label record stored in the `character_contact_labels` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "character_contact_labels")]
pub struct Model {
  /// ID of the owning character.
  pub character_id: i64,
  /// Auto-increment primary key.
  #[sea_orm(primary_key)]
  pub id: i32,
  /// EVE label ID.
  pub label_id: i32,
  /// Display name of the label.
  pub label_name: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
