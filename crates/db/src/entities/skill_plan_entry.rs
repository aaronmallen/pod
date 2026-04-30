//! Database entity for skill plan entries.

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
