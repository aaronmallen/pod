//! Domain model for an EVE Online faction.

use getset::Getters;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// An EVE Online faction with change-tracking for database persistence.
#[derive(Clone, Debug, Deserialize, Getters, PartialEq, Serialize, Validate)]
pub struct Model {
  /// Human-readable description of the faction.
  #[get = "pub"]
  description: String,
  dirty: bool,
  /// Unique faction identifier.
  #[get = "pub"]
  id: i32,
  /// Whether this faction is unique (singleton) within the universe.
  #[get = "pub"]
  is_unique: bool,
  /// Display name of the faction.
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  persisted: bool,
  /// Relative size factor used for territorial calculations.
  #[get = "pub"]
  size_factor: f64,
  /// ID of the faction's home solar system, if any.
  #[get = "pub"]
  solar_system_id: Option<i32>,
}

impl Model {
  /// Creates a new, unpersisted faction with the given ID and name.
  pub fn new(id: i32, name: impl Into<String>) -> Self {
    Self {
      description: String::new(),
      dirty: false,
      id,
      is_unique: false,
      name: name.into(),
      persisted: false,
      size_factor: 1.0,
      solar_system_id: None,
    }
  }

  /// Returns `true` if any field has been mutated since the model was last persisted.
  pub fn has_changes(&self) -> bool {
    self.dirty
  }

  /// Returns `true` if this model was loaded from or has been saved to the database.
  pub fn is_persisted(&self) -> bool {
    self.persisted
  }

  /// Sets the faction description, flagging the model dirty if already persisted.
  pub fn set_description(&mut self, description: impl Into<String>) -> &mut Self {
    self.description = description.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the is_unique flag, flagging the model dirty if already persisted.
  pub fn set_is_unique(&mut self, is_unique: bool) -> &mut Self {
    self.is_unique = is_unique;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the faction name, flagging the model dirty if already persisted.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the size factor, flagging the model dirty if already persisted.
  pub fn set_size_factor(&mut self, size_factor: f64) -> &mut Self {
    self.size_factor = size_factor;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the home solar system ID, flagging the model dirty if already persisted.
  pub fn set_solar_system_id(&mut self, solar_system_id: Option<i32>) -> &mut Self {
    self.solar_system_id = solar_system_id;
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
      let mut f = Model::new(500_001, "Caldari State");
      f.set_size_factor(0.5);
      assert!(!f.has_changes());
    }

    #[test]
    fn it_returns_true_after_persist_and_mutation() {
      let mut f = Model::new(500_001, "Caldari State");
      f.mark_persisted();
      f.set_size_factor(0.5);
      assert!(f.has_changes());
    }
  }

  mod mark_persisted {
    use super::*;

    #[test]
    fn it_marks_model_as_persisted_without_dirtying() {
      let mut f = Model::new(500_001, "Caldari State");
      f.mark_persisted();
      assert!(f.is_persisted());
      assert!(!f.has_changes());
    }
  }

  mod set_description {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut f = Model::new(500_001, "Caldari State");
      f.mark_persisted();
      f.set_description("A corporate state.");
      assert!(f.has_changes());
    }
  }

  mod set_is_unique {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut f = Model::new(500_001, "Caldari State");
      f.mark_persisted();
      f.set_is_unique(true);
      assert!(f.has_changes());
    }
  }

  mod set_name {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut f = Model::new(500_001, "Caldari State");
      f.mark_persisted();
      f.set_name("Updated Name");
      assert!(f.has_changes());
    }
  }

  mod set_solar_system_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut f = Model::new(500_001, "Caldari State");
      f.mark_persisted();
      f.set_solar_system_id(Some(30_000_142));
      assert!(f.has_changes());
    }
  }
}
