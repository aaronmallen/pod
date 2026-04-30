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
