//! Skill type re-exports and lookup helpers.

pub use pod_model::{AttrKey, SkillDef, SkillGroupDef};

/// Look up a skill by name across the given groups. Returns `(skill, group_name)`.
pub fn find_skill<'a>(name: &str, groups: &'a [SkillGroupDef]) -> Option<(&'a SkillDef, &'a str)> {
  for g in groups {
    if let Some(s) = g.skills.iter().find(|s| s.name == name) {
      return Some((s, &g.name));
    }
  }
  None
}
