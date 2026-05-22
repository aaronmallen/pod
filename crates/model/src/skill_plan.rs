//! Domain model for skill plans.

/// A named skill training plan belonging to a character.
#[derive(Clone, Debug)]
pub struct SkillPlan {
  /// EVE character ID that owns this plan.
  pub character_id: i64,
  /// Unix timestamp when the plan was created.
  pub created_at: i64,
  /// Ordered list of skill entries in this plan.
  pub entries: Vec<super::SkillPlanEntry>,
  /// Unique identifier for this plan (UUID string).
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

#[cfg(test)]
mod tests {
  mod skill_plan {
    use pretty_assertions::assert_eq;

    use super::super::*;

    #[test]
    fn it_stores_fields() {
      let plan = SkillPlan {
        character_id: 90_000_001,
        created_at: 1_700_000_000,
        entries: vec![],
        id: "plan-uuid".into(),
        implant_set: "standard".into(),
        name: "PVP Plan".into(),
        remap_json: None,
        updated_at: 1_700_000_001,
      };

      assert_eq!(plan.character_id, 90_000_001);
      assert_eq!(plan.name, "PVP Plan");
      assert!(plan.entries.is_empty());
      assert!(plan.remap_json.is_none());
    }

    #[test]
    fn it_accepts_remap_json() {
      let plan = SkillPlan {
        character_id: 90_000_001,
        created_at: 0,
        entries: vec![],
        id: "id".into(),
        implant_set: "none".into(),
        name: "Remap Plan".into(),
        remap_json: Some(r#"{"perception":27}"#.into()),
        updated_at: 0,
      };

      assert!(plan.remap_json.is_some());
    }
  }
}
