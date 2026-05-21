//! Database entity for EVE certificate definitions.

use sea_orm::prelude::*;

/// A certificate definition stored in the `certificates` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "certificates")]
pub struct Model {
  /// Optional prose description of the certificate.
  pub description: Option<String>,
  /// Mastery grade level (0–4).
  pub grade: i32,
  /// Unique certificate identifier.
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: i32,
  /// Display name.
  pub name: String,
  /// JSON array of skill requirements.
  pub skills_json: String,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
