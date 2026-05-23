//! Domain model for an EVE Online solar system.

use getset::{Getters, MutGetters};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// A solar system record, with optional associations to its planets, stargates, and stations.
///
/// Tracks whether the record has been persisted to the database and whether unsaved
/// changes are present via the `dirty` flag.
#[derive(Clone, Debug, Deserialize, Getters, MutGetters, PartialEq, Serialize, Validate)]
pub struct Model {
  /// Parent constellation, if loaded.
  #[get = "pub"]
  constellation: Option<crate::constellation::Model>,
  /// Parent constellation identifier.
  #[get = "pub"]
  constellation_id: i32,
  dirty: bool,
  /// Unique solar system identifier.
  #[get = "pub"]
  id: i32,
  /// Display name of the solar system.
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  persisted: bool,
  /// Planets orbiting within this solar system.
  #[getset(get = "pub", get_mut = "pub")]
  planets: Vec<crate::planet::Model>,
  /// X coordinate of the solar system's position in space (metres).
  #[get = "pub"]
  position_x: f64,
  /// Y coordinate of the solar system's position in space (metres).
  #[get = "pub"]
  position_y: f64,
  /// Z coordinate of the solar system's position in space (metres).
  #[get = "pub"]
  position_z: f64,
  /// Security classification label (e.g. "A", "B"), if assigned.
  #[get = "pub"]
  security_class: Option<String>,
  /// Numeric security status of the solar system (typically -1.0 to 1.0).
  #[get = "pub"]
  security_status: f64,
  /// Optional star ID for the star at the center of this solar system.
  #[get = "pub"]
  star_id: Option<i32>,
  /// Stargates within this solar system.
  #[getset(get = "pub", get_mut = "pub")]
  stargates: Vec<crate::stargate::Model>,
  /// Stations within this solar system.
  #[getset(get = "pub", get_mut = "pub")]
  stations: Vec<crate::station::Model>,
}

impl Model {
  /// Creates a new, unpersisted solar system with the given ID and name.
  ///
  /// All numeric fields default to zero, optional fields to `None`, and
  /// association collections to empty.
  pub fn new(id: i32, name: impl Into<String>) -> Self {
    Self {
      constellation: None,
      constellation_id: 0,
      dirty: false,
      id,
      name: name.into(),
      persisted: false,
      planets: Vec::new(),
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      security_class: None,
      security_status: 0.0,
      star_id: None,
      stargates: Vec::new(),
      stations: Vec::new(),
    }
  }

  /// Returns `true` if any field has been mutated since the record was last saved.
  pub fn has_changes(&self) -> bool {
    self.dirty
  }

  /// Returns `true` if this record was loaded from or has been saved to the database.
  pub fn is_persisted(&self) -> bool {
    self.persisted
  }

  /// Sets the eagerly loaded constellation record.
  pub fn set_constellation(&mut self, constellation: Option<crate::constellation::Model>) -> &mut Self {
    self.constellation = constellation;
    self
  }

  /// Sets the parent constellation ID, marking the record dirty if already persisted.
  pub fn set_constellation_id(&mut self, constellation_id: i32) -> &mut Self {
    self.constellation_id = constellation_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the solar system name, marking the record dirty if already persisted.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the 3-D position coordinates, marking the record dirty if already persisted.
  pub fn set_position(&mut self, x: f64, y: f64, z: f64) -> &mut Self {
    self.position_x = x;
    self.position_y = y;
    self.position_z = z;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the security class string, marking the record dirty if already persisted.
  pub fn set_security_class(&mut self, security_class: Option<String>) -> &mut Self {
    self.security_class = security_class;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the numeric security status, marking the record dirty if already persisted.
  pub fn set_security_status(&mut self, security_status: f64) -> &mut Self {
    self.security_status = security_status;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the optional star ID, marking the record dirty if already persisted.
  pub fn set_star_id(&mut self, star_id: Option<i32>) -> &mut Self {
    self.star_id = star_id;
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
      let mut s = Model::new(30_000_142, "Jita");
      s.set_security_status(0.9459);
      assert!(!s.has_changes());
    }

    #[test]
    fn it_returns_true_after_persist_and_mutation() {
      let mut s = Model::new(30_000_142, "Jita");
      s.mark_persisted();
      s.set_security_status(0.9459);
      assert!(s.has_changes());
    }
  }

  mod mark_persisted {
    use super::*;

    #[test]
    fn it_marks_model_as_persisted_without_dirtying() {
      let mut s = Model::new(30_000_142, "Jita");
      s.mark_persisted();
      assert!(s.is_persisted());
      assert!(!s.has_changes());
    }
  }

  mod set_constellation_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut s = Model::new(30_000_142, "Jita");
      s.mark_persisted();
      s.set_constellation_id(20_000_020);
      assert!(s.has_changes());
    }
  }

  mod set_position {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut s = Model::new(30_000_142, "Jita");
      s.mark_persisted();
      s.set_position(1.0, 2.0, 3.0);
      assert!(s.has_changes());
    }
  }

  mod set_security_class {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut s = Model::new(30_000_142, "Jita");
      s.mark_persisted();
      s.set_security_class(Some("A".into()));
      assert!(s.has_changes());
    }
  }

  mod set_star_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut s = Model::new(30_000_142, "Jita");
      s.mark_persisted();
      s.set_star_id(Some(40_009_080));
      assert!(s.has_changes());
    }
  }
}
