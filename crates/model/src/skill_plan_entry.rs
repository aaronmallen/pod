//! Domain model for skill plan entries.

/// A single skill training entry within a skill plan.
#[derive(Clone, Debug)]
pub struct SkillPlanEntry {
  /// Whether this entry was automatically inserted as a prerequisite.
  pub auto: bool,
  /// Unique identifier for this entry (UUID string).
  pub id: String,
  /// Optional user note attached to this entry.
  pub note: Option<String>,
  /// FK to the owning skill plan.
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
