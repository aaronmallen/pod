//! Domain model for abyssal module stat bounds (from dynamicItemAttributes).

use getset::Getters;
use serde::{Deserialize, Serialize};

/// The min/max multiplier bounds for a single dogma attribute on an abyssal
/// module type, as defined in the SDE `dynamicItemAttributes.yaml`.
#[derive(Clone, Debug, Deserialize, Getters, PartialEq, Serialize)]
pub struct AbyssalModuleStat {
  /// The resulting abyssal type ID (the mutated item type).
  #[get = "pub"]
  abyssal_type_id: i32,
  /// The dogma attribute ID this bound applies to.
  #[get = "pub"]
  attribute_id: i32,
  /// Maximum multiplier that can be applied to the base stat.
  #[get = "pub"]
  max_mult: f64,
  /// Minimum multiplier that can be applied to the base stat.
  #[get = "pub"]
  min_mult: f64,
}

impl AbyssalModuleStat {
  /// Creates a new `AbyssalModuleStat`.
  pub fn new(abyssal_type_id: i32, attribute_id: i32, min_mult: f64, max_mult: f64) -> Self {
    Self {
      abyssal_type_id,
      attribute_id,
      max_mult,
      min_mult,
    }
  }
}
