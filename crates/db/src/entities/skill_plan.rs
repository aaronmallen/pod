//! Database entity for skill plans.

use sea_orm::prelude::*;

/// A skill plan stored in the `skill_plans` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "skill_plans")]
pub struct Model {
  /// ID of the owning character.
  pub character_id: i64,
  /// Unix timestamp when the plan was created.
  pub created_at: i64,
  /// Text primary key (UUID).
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: String,
  /// Implant set to assume when computing training times.
  pub implant_set: String,
  /// User-assigned name for this skill plan.
  pub name: String,
  /// Optional serialized remap attributes (JSON).
  pub remap_json: Option<String>,
  /// Unix timestamp when the plan was last updated.
  pub updated_at: i64,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}
