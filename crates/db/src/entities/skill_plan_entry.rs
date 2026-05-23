//! Database entity for skill plan entries.

use pod_model::SkillPlanEntry;
use sea_orm::prelude::*;

/// A single skill training entry stored in the `skill_plan_entries` table.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "skill_plan_entries")]
pub struct Model {
  /// Whether this entry was automatically inserted as a prerequisite (0 = false, 1 = true).
  pub auto: i32,
  /// Text primary key (UUID).
  #[sea_orm(primary_key, auto_increment = false)]
  pub id: String,
  /// Optional user note attached to this entry.
  pub note: Option<String>,
  /// The skill plan that owns this entry.
  #[sea_orm(belongs_to, from = "plan_id", to = "id")]
  pub plan: HasOne<super::skill_plan::Entity>,
  /// FK to the owning skill plan in `skill_plans`.
  pub plan_id: String,
  /// Display order of this entry within the plan.
  pub position: i32,
  /// Priority label (e.g. "normal", "high").
  pub priority: String,
  /// Resolved display name of the skill.
  pub skill_name: String,
  /// Target trained level (1–5).
  pub to_level: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<Model> for SkillPlanEntry {
  fn from(entity: Model) -> Self {
    Self {
      auto: entity.auto != 0,
      id: entity.id,
      note: entity.note,
      plan_id: entity.plan_id,
      position: entity.position,
      priority: entity.priority,
      skill_name: entity.skill_name,
      to_level: entity.to_level,
    }
  }
}

impl From<ModelEx> for SkillPlanEntry {
  fn from(entity: ModelEx) -> Self {
    Self {
      auto: entity.auto != 0,
      id: entity.id,
      note: entity.note,
      plan_id: entity.plan_id,
      position: entity.position,
      priority: entity.priority,
      skill_name: entity.skill_name,
      to_level: entity.to_level,
    }
  }
}
