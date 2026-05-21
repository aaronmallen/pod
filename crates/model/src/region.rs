//! Domain model for EVE Online regions.

use getset::{Getters, MutGetters};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// A region in the EVE Online universe, optionally holding its child constellations.
///
/// Tracks whether the record has been persisted to the database and whether any
/// fields have been mutated since the last save (`dirty` flag).
#[derive(Clone, Debug, Deserialize, Getters, MutGetters, PartialEq, Serialize, Validate)]
pub struct Model {
  /// Child constellations, populated when loaded via eager loading.
  #[getset(get = "pub", get_mut = "pub")]
  constellations: Vec<crate::constellation::Model>,
  /// Optional lore description for the region.
  #[get = "pub"]
  description: Option<String>,
  dirty: bool,
  /// Unique region identifier.
  #[get = "pub"]
  id: i32,
  /// Display name of the region.
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  persisted: bool,
}

impl Model {
  /// Creates a new, unpersisted region with the given id and name.
  pub fn new(id: i32, name: impl Into<String>) -> Self {
    Self {
      constellations: Vec::new(),
      description: None,
      dirty: false,
      id,
      name: name.into(),
      persisted: false,
    }
  }

  /// Returns `true` if any field has been modified since the model was last persisted.
  pub fn has_changes(&self) -> bool {
    self.dirty
  }

  /// Returns `true` if this model was loaded from or successfully saved to the database.
  pub fn is_persisted(&self) -> bool {
    self.persisted
  }

  /// Sets the description, marking the model dirty if it has already been persisted.
  pub fn set_description(&mut self, description: Option<String>) -> &mut Self {
    self.description = description;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the name, marking the model dirty if it has already been persisted.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Marks this model as loaded from the database without affecting the dirty flag.
  pub fn mark_persisted(&mut self) -> &mut Self {
    self.persisted = true;
    self
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod has_changes {
    use super::*;

    #[test]
    fn it_returns_false_before_persist() {
      let mut r = Model::new(10_000_002, "The Forge");
      r.set_description(Some("A region of New Eden.".into()));
      assert!(!r.has_changes());
    }

    #[test]
    fn it_returns_true_after_persist_and_mutation() {
      let mut r = Model::new(10_000_002, "The Forge");
      r.mark_persisted();
      r.set_description(Some("A region of New Eden.".into()));
      assert!(r.has_changes());
    }
  }

  mod mark_persisted {
    use super::*;

    #[test]
    fn it_marks_model_as_persisted_without_dirtying() {
      let mut r = Model::new(10_000_002, "The Forge");
      r.mark_persisted();
      assert!(r.is_persisted());
      assert!(!r.has_changes());
    }
  }
}
