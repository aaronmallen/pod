//! Database entity for skill plans.

use pod_model::SkillPlan;
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
  /// Entries belonging to this skill plan.
  #[sea_orm(has_many)]
  pub entries: HasMany<super::skill_plan_entry::Entity>,
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

impl From<Model> for SkillPlan {
  fn from(entity: Model) -> Self {
    Self {
      character_id: entity.character_id,
      created_at: entity.created_at,
      entries: vec![],
      id: entity.id,
      implant_set: entity.implant_set,
      name: entity.name,
      remap_json: entity.remap_json,
      updated_at: entity.updated_at,
    }
  }
}

impl From<ModelEx> for SkillPlan {
  fn from(entity: ModelEx) -> Self {
    Self {
      character_id: entity.character_id,
      created_at: entity.created_at,
      entries: entity.entries.into_iter().map(Into::into).collect(),
      id: entity.id,
      implant_set: entity.implant_set,
      name: entity.name,
      remap_json: entity.remap_json,
      updated_at: entity.updated_at,
    }
  }
}
