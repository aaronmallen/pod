//! Domain model for a stargate connecting two solar systems.

use getset::Getters;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// A stargate that links one solar system to another.
///
/// Tracks whether the record has been saved to the database (`persisted`) and
/// whether any field has been mutated since it was loaded (`dirty`).
#[derive(Clone, Debug, Deserialize, Getters, PartialEq, Serialize, Validate)]
pub struct Model {
  /// ID of the solar system reached through this stargate.
  #[get = "pub"]
  destination_solar_system_id: i32,
  /// ID of the stargate on the other end of this connection.
  #[get = "pub"]
  destination_stargate_id: i32,
  dirty: bool,
  /// Unique stargate identifier.
  #[get = "pub"]
  id: i32,
  /// EVE item-type ID that describes this stargate's type.
  #[get = "pub"]
  item_type_id: i32,
  /// Display name of the stargate.
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  persisted: bool,
  /// X coordinate of the stargate's position in its solar system (metres).
  #[get = "pub"]
  position_x: f64,
  /// Y coordinate of the stargate's position in its solar system (metres).
  #[get = "pub"]
  position_y: f64,
  /// Z coordinate of the stargate's position in its solar system (metres).
  #[get = "pub"]
  position_z: f64,
  /// ID of the solar system that contains this stargate.
  #[get = "pub"]
  solar_system_id: i32,
}

impl Model {
  /// Creates a new, unpersisted stargate with the given `id` and `name`.
  ///
  /// All numeric fields default to `0` and position fields default to `0.0`.
  pub fn new(id: i32, name: impl Into<String>) -> Self {
    Self {
      destination_solar_system_id: 0,
      destination_stargate_id: 0,
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

  /// Returns `true` if any field has been mutated since the model was loaded or created.
  pub fn has_changes(&self) -> bool {
    self.dirty
  }

  /// Returns `true` if this model has been saved to the database.
  pub fn is_persisted(&self) -> bool {
    self.persisted
  }

  /// Sets the destination stargate and solar system for this gate.
  ///
  /// Marks the model dirty when it has already been persisted.
  pub fn set_destination(&mut self, stargate_id: i32, solar_system_id: i32) -> &mut Self {
    self.destination_stargate_id = stargate_id;
    self.destination_solar_system_id = solar_system_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the EVE item-type ID for this stargate.
  pub fn set_item_type_id(&mut self, item_type_id: i32) -> &mut Self {
    self.item_type_id = item_type_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the display name of this stargate.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the 3-D position of this stargate within its solar system (metres).
  pub fn set_position(&mut self, x: f64, y: f64, z: f64) -> &mut Self {
    self.position_x = x;
    self.position_y = y;
    self.position_z = z;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the ID of the solar system that contains this stargate.
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
