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

#[cfg(test)]
mod tests {
  mod skill_plan_entry {
    use pretty_assertions::assert_eq;

    use super::super::*;

    #[test]
    fn it_stores_fields() {
      let entry = SkillPlanEntry {
        auto: false,
        id: "entry-uuid".into(),
        note: Some("Core skill".into()),
        plan_id: "plan-uuid".into(),
        position: 1,
        priority: "normal".into(),
        skill_name: "Caldari Frigate".into(),
        to_level: 4,
      };

      assert_eq!(entry.skill_name, "Caldari Frigate");
      assert_eq!(entry.to_level, 4);
      assert_eq!(entry.position, 1);
      assert!(!entry.auto);
    }

    #[test]
    fn it_accepts_auto_prerequisites_with_no_note() {
      let entry = SkillPlanEntry {
        auto: true,
        id: "entry-uuid-2".into(),
        note: None,
        plan_id: "plan-uuid".into(),
        position: 0,
        priority: "high".into(),
        skill_name: "Spaceship Command".into(),
        to_level: 1,
      };

      assert!(entry.auto);
      assert!(entry.note.is_none());
    }
  }
}
