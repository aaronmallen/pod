//! Lightweight summary model for item types returned by ship/module searches.

/// A condensed view of an item type for use in ship and module search results.
#[derive(Clone, Debug)]
pub struct ItemTypeSummary {
  /// The unique type ID.
  pub id: i32,
  /// The display name of the type.
  pub name: String,
  /// The name of the item group this type belongs to.
  pub group_name: String,
  /// Skill requirements resolved from dogma attributes: `(skill_name, level)`.
  pub skill_requirements: Vec<(String, u8)>,
  /// Certificate IDs required per mastery tier (index 0 = tier I, …, 4 = tier V).
  /// Ships only; empty for modules.
  pub mastery_cert_ids: Vec<Vec<i32>>,
}

#[cfg(test)]
mod tests {
  mod item_type_summary {
    use pretty_assertions::assert_eq;

    use super::super::*;

    #[test]
    fn it_stores_fields() {
      let summary = ItemTypeSummary {
        id: 587,
        name: "Rifter".into(),
        group_name: "Frigate".into(),
        skill_requirements: vec![("Minmatar Frigate".into(), 1)],
        mastery_cert_ids: vec![vec![1, 2], vec![3]],
      };

      assert_eq!(summary.id, 587);
      assert_eq!(summary.name, "Rifter");
      assert_eq!(summary.group_name, "Frigate");
      assert_eq!(summary.skill_requirements.len(), 1);
      assert_eq!(summary.skill_requirements[0], ("Minmatar Frigate".to_string(), 1));
    }

    #[test]
    fn it_accepts_empty_requirements_for_modules() {
      let summary = ItemTypeSummary {
        id: 3178,
        name: "200mm AutoCannon II".into(),
        group_name: "Auto Cannon".into(),
        skill_requirements: vec![],
        mastery_cert_ids: vec![],
      };

      assert!(summary.skill_requirements.is_empty());
      assert!(summary.mastery_cert_ids.is_empty());
    }
  }
}
