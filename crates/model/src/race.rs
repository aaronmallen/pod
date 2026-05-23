//! Domain model for EVE Online races.

use getset::{Getters, MutGetters};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// A playable race in EVE Online, grouping related bloodlines under a named faction heritage.
///
/// Tracks dirty/persisted state so callers can detect unsaved changes before writing to the
/// database.
#[derive(Clone, Debug, Deserialize, Getters, MutGetters, PartialEq, Serialize, Validate)]
pub struct Model {
  /// ID of the NPC alliance associated with this race.
  #[get = "pub"]
  alliance_id: i32,
  /// Bloodlines belonging to this race, populated via eager loading.
  #[getset(get = "pub", get_mut = "pub")]
  bloodlines: Vec<crate::bloodline::Model>,
  /// Lore description of the race.
  #[get = "pub"]
  description: String,
  dirty: bool,
  /// Unique race identifier.
  #[get = "pub"]
  id: i32,
  /// Display name of the race.
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  persisted: bool,
}

impl Model {
  /// Creates a new unpersisted race with default field values.
  pub fn new(id: i32, name: impl Into<String>) -> Self {
    Self {
      alliance_id: 0,
      bloodlines: Vec::new(),
      description: String::new(),
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

  /// Returns `true` if this model was loaded from the database.
  pub fn is_persisted(&self) -> bool {
    self.persisted
  }

  /// Sets the alliance ID, marking the model dirty if already persisted.
  pub fn set_alliance_id(&mut self, alliance_id: i32) -> &mut Self {
    self.alliance_id = alliance_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the description, marking the model dirty if already persisted.
  pub fn set_description(&mut self, description: impl Into<String>) -> &mut Self {
    self.description = description.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the display name, marking the model dirty if already persisted.
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
      let mut r = Model::new(1, "Caldari");
      r.set_description("A corporate meritocracy.");
      assert!(!r.has_changes());
    }

    #[test]
    fn it_returns_true_after_persist_and_mutation() {
      let mut r = Model::new(1, "Caldari");
      r.mark_persisted();
      r.set_description("A corporate meritocracy.");
      assert!(r.has_changes());
    }
  }

  mod mark_persisted {
    use super::*;

    #[test]
    fn it_marks_model_as_persisted_without_dirtying() {
      let mut r = Model::new(1, "Caldari");
      r.mark_persisted();
      assert!(r.is_persisted());
      assert!(!r.has_changes());
    }
  }

  mod set_alliance_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut r = Model::new(1, "Caldari");
      r.mark_persisted();
      r.set_alliance_id(500_001);
      assert!(r.has_changes());
    }
  }

  mod set_name {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut r = Model::new(1, "Caldari");
      r.mark_persisted();
      r.set_name("Caldari State");
      assert!(r.has_changes());
    }
  }
}
