//! Database entity for ship mastery certificate mappings.

use sea_orm::prelude::*;

/// A ship mastery entry stored in the `ship_mastery_certs` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "ship_mastery_certs")]
pub struct Model {
  /// JSON array of certificate IDs required for this mastery level.
  pub cert_ids_json: String,
  /// Mastery level (1–5); part of the composite primary key.
  #[sea_orm(primary_key, auto_increment = false)]
  pub mastery_level: i32,
  /// EVE type ID of the ship; part of the composite primary key.
  #[sea_orm(primary_key, auto_increment = false)]
  pub ship_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
