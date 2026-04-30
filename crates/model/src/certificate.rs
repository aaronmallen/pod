//! Domain model for EVE Online certificates used in ship mastery tiers.

/// A single EVE Online certificate, defining required skills for a mastery tier.
#[derive(Clone, Debug)]
pub struct Certificate {
  pub id: i32,
  pub name: String,
  pub description: Option<String>,
  pub grade: u8,
  /// `(type_id, [basic, improved, advanced, elite])` — required skill level at each proficiency tier.
  pub skills: Vec<(i32, [u8; 4])>,
}
