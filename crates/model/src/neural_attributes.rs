//! Lightweight neural attribute value structs shared between the DB and UI
//! layers.

/// Five core neural attributes for a character or an implant bonus.
///
/// Used as the return type for both character effective attributes and active-
/// clone implant bonus calculations.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NeuralAttributes {
  /// Charisma attribute value.
  pub charisma: i32,
  /// Intelligence attribute value.
  pub intelligence: i32,
  /// Memory attribute value.
  pub memory: i32,
  /// Perception attribute value.
  pub perception: i32,
  /// Willpower attribute value.
  pub willpower: i32,
}
