//! Domain model for a star within a solar system.

use getset::Getters;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// A star belonging to a solar system.
///
/// Tracks whether the record has been persisted to the database and whether any
/// field has been mutated since it was last saved (`dirty` flag).
#[derive(Clone, Debug, Deserialize, Getters, PartialEq, Serialize, Validate)]
pub struct Model {
  /// Age of the star in years.
  #[get = "pub"]
  age: i64,
  dirty: bool,
  /// Unique star identifier.
  #[get = "pub"]
  id: i32,
  /// EVE item type ID for this star.
  #[get = "pub"]
  item_type_id: i32,
  /// Luminosity relative to the standard solar luminosity.
  #[get = "pub"]
  luminosity: f64,
  /// Display name of the star.
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  persisted: bool,
  /// Radius of the star in metres.
  #[get = "pub"]
  radius: i64,
  /// Parent solar system identifier.
  #[get = "pub"]
  solar_system_id: i32,
  /// Harvard spectral classification (e.g. `"G2V"`).
  #[get = "pub"]
  spectral_class: String,
  /// Surface temperature in Kelvin.
  #[get = "pub"]
  temperature: i32,
}

impl Model {
  /// Creates a new, unsaved star with default zero values for all physical properties.
  pub fn new(id: i32, name: impl Into<String>) -> Self {
    Self {
      age: 0,
      dirty: false,
      id,
      item_type_id: 0,
      luminosity: 0.0,
      name: name.into(),
      persisted: false,
      radius: 0,
      solar_system_id: 0,
      spectral_class: String::new(),
      temperature: 0,
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

  /// Sets the star's age and marks the record dirty if it has already been persisted.
  pub fn set_age(&mut self, age: i64) -> &mut Self {
    self.age = age;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the item type ID and marks the record dirty if it has already been persisted.
  pub fn set_item_type_id(&mut self, item_type_id: i32) -> &mut Self {
    self.item_type_id = item_type_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the luminosity and marks the record dirty if it has already been persisted.
  pub fn set_luminosity(&mut self, luminosity: f64) -> &mut Self {
    self.luminosity = luminosity;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the star's name and marks the record dirty if it has already been persisted.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the radius and marks the record dirty if it has already been persisted.
  pub fn set_radius(&mut self, radius: i64) -> &mut Self {
    self.radius = radius;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the owning solar system ID and marks the record dirty if it has already been persisted.
  pub fn set_solar_system_id(&mut self, solar_system_id: i32) -> &mut Self {
    self.solar_system_id = solar_system_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the spectral class and marks the record dirty if it has already been persisted.
  pub fn set_spectral_class(&mut self, spectral_class: impl Into<String>) -> &mut Self {
    self.spectral_class = spectral_class.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the surface temperature and marks the record dirty if it has already been persisted.
  pub fn set_temperature(&mut self, temperature: i32) -> &mut Self {
    self.temperature = temperature;
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
      let mut s = Model::new(40_009_082, "Jita IV - Moon 4 - Caldari Navy Assembly Plant");
      s.set_age(4_600_000_000);
      assert!(!s.has_changes());
    }

    #[test]
    fn it_returns_true_after_persist_and_mutation() {
      let mut s = Model::new(40_009_082, "Jita IV - Moon 4 - Caldari Navy Assembly Plant");
      s.mark_persisted();
      s.set_age(4_600_000_000);
      assert!(s.has_changes());
    }
  }

  mod mark_persisted {
    use super::*;

    #[test]
    fn it_marks_model_as_persisted_without_dirtying() {
      let mut s = Model::new(40_009_082, "Sol");
      s.mark_persisted();
      assert!(s.is_persisted());
      assert!(!s.has_changes());
    }
  }

  mod set_item_type_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut s = Model::new(40_009_082, "Sol");
      s.mark_persisted();
      s.set_item_type_id(3800);
      assert!(s.has_changes());
    }
  }

  mod set_luminosity {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut s = Model::new(40_009_082, "Sol");
      s.mark_persisted();
      s.set_luminosity(1.0);
      assert!(s.has_changes());
    }
  }

  mod set_radius {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut s = Model::new(40_009_082, "Sol");
      s.mark_persisted();
      s.set_radius(695_700_000);
      assert!(s.has_changes());
    }
  }

  mod set_solar_system_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut s = Model::new(40_009_082, "Sol");
      s.mark_persisted();
      s.set_solar_system_id(30_000_142);
      assert!(s.has_changes());
    }
  }

  mod set_spectral_class {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut s = Model::new(40_009_082, "Sol");
      s.mark_persisted();
      s.set_spectral_class("G2V");
      assert!(s.has_changes());
    }
  }

  mod set_temperature {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut s = Model::new(40_009_082, "Sol");
      s.mark_persisted();
      s.set_temperature(5_778);
      assert!(s.has_changes());
    }
  }
}
