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
