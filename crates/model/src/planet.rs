//! Domain model for a planet within a solar system.

use getset::Getters;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// A planet orbiting within a solar system.
///
/// Tracks identity, position, and item-type classification. The `dirty` flag
/// is set whenever a field is mutated after the record has been persisted to
/// the database, signalling that an update is required.
#[derive(Clone, Debug, Deserialize, Getters, PartialEq, Serialize, Validate)]
pub struct Model {
  dirty: bool,
  /// Unique planet identifier.
  #[get = "pub"]
  id: i32,
  /// EVE item type ID for this planet.
  #[get = "pub"]
  item_type_id: i32,
  /// Display name of the planet.
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  persisted: bool,
  /// X coordinate of the planet's position in the solar system (metres).
  #[get = "pub"]
  position_x: f64,
  /// Y coordinate of the planet's position in the solar system (metres).
  #[get = "pub"]
  position_y: f64,
  /// Z coordinate of the planet's position in the solar system (metres).
  #[get = "pub"]
  position_z: f64,
  /// Parent solar system identifier.
  #[get = "pub"]
  solar_system_id: i32,
}

impl Model {
  /// Creates a new, unpersisted planet with the given ID and name.
  ///
  /// All numeric fields default to zero; use the `set_*` methods to
  /// populate them before saving.
  pub fn new(id: i32, name: impl Into<String>) -> Self {
    Self {
      dirty: false,
      id,
      item_type_id: 0,
      name: name.into(),
      persisted: false,
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      solar_system_id: 0,
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

  /// Sets the item type ID, marking the model dirty if it is already persisted.
  pub fn set_item_type_id(&mut self, item_type_id: i32) -> &mut Self {
    self.item_type_id = item_type_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the planet name, marking the model dirty if it is already persisted.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the 3-D position coordinates, marking the model dirty if it is already persisted.
  pub fn set_position(&mut self, x: f64, y: f64, z: f64) -> &mut Self {
    self.position_x = x;
    self.position_y = y;
    self.position_z = z;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the parent solar system ID, marking the model dirty if it is already persisted.
  pub fn set_solar_system_id(&mut self, solar_system_id: i32) -> &mut Self {
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
      let mut p = Model::new(40_009_081, "Jita IV");
      p.set_solar_system_id(30_000_142);
      assert!(!p.has_changes());
    }

    #[test]
    fn it_returns_true_after_persist_and_mutation() {
      let mut p = Model::new(40_009_081, "Jita IV");
      p.mark_persisted();
      p.set_solar_system_id(30_000_142);
      assert!(p.has_changes());
    }
  }

  mod mark_persisted {
    use super::*;

    #[test]
    fn it_marks_model_as_persisted_without_dirtying() {
      let mut p = Model::new(40_009_081, "Jita IV");
      p.mark_persisted();
      assert!(p.is_persisted());
      assert!(!p.has_changes());
    }
  }

  mod set_item_type_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut p = Model::new(40_009_081, "Jita IV");
      p.mark_persisted();
      p.set_item_type_id(13);
      assert!(p.has_changes());
    }
  }

  mod set_name {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut p = Model::new(40_009_081, "Jita IV");
      p.mark_persisted();
      p.set_name("Updated Planet");
      assert!(p.has_changes());
    }
  }

  mod set_position {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut p = Model::new(40_009_081, "Jita IV");
      p.mark_persisted();
      p.set_position(1.0, 2.0, 3.0);
      assert!(p.has_changes());
    }
  }
}
