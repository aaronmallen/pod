//! Domain model for EVE Online constellations.

use getset::{Getters, MutGetters};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// A constellation grouping one or more solar systems within a region.
///
/// Tracks whether the record has been persisted to the database and whether any
/// fields have been mutated since the last save (`dirty` flag).
#[derive(Clone, Debug, Deserialize, Getters, MutGetters, PartialEq, Serialize, Validate)]
pub struct Model {
  dirty: bool,
  /// Unique constellation identifier.
  #[get = "pub"]
  id: i32,
  /// Display name of the constellation.
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  persisted: bool,
  /// X coordinate of the constellation's reference position (metres).
  #[get = "pub"]
  position_x: f64,
  /// Y coordinate of the constellation's reference position (metres).
  #[get = "pub"]
  position_y: f64,
  /// Z coordinate of the constellation's reference position (metres).
  #[get = "pub"]
  position_z: f64,
  /// Parent region identifier.
  #[get = "pub"]
  region_id: i32,
  /// Child solar systems, populated when loaded via eager loading.
  #[getset(get = "pub", get_mut = "pub")]
  solar_systems: Vec<crate::solar_system::Model>,
}

impl Model {
  /// Creates a new unpersisted constellation with the given ID and name.
  pub fn new(id: i32, name: impl Into<String>) -> Self {
    Self {
      dirty: false,
      id,
      name: name.into(),
      persisted: false,
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      region_id: 0,
      solar_systems: Vec::new(),
    }
  }

  /// Returns `true` if any field has been modified since the model was last persisted.
  pub fn has_changes(&self) -> bool {
    self.dirty
  }

  /// Returns `true` if this model was loaded from or saved to the database.
  pub fn is_persisted(&self) -> bool {
    self.persisted
  }

  /// Sets the constellation name, marking the model dirty if already persisted.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the 3-D position of the constellation in universe coordinates, marking the model dirty if already persisted.
  pub fn set_position(&mut self, x: f64, y: f64, z: f64) -> &mut Self {
    self.position_x = x;
    self.position_y = y;
    self.position_z = z;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the parent region ID, marking the model dirty if already persisted.
  pub fn set_region_id(&mut self, region_id: i32) -> &mut Self {
    self.region_id = region_id;
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
