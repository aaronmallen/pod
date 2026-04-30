//! Domain model for NPC stations.

use getset::{Getters, MutGetters};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// An NPC station within a solar system.
///
/// Tracks whether the record has been persisted to the database and whether any
/// field has been mutated since it was last saved (`dirty` flag).
#[derive(Clone, Debug, Deserialize, Getters, MutGetters, PartialEq, Serialize, Validate)]
pub struct Model {
  dirty: bool,
  /// Unique station identifier.
  #[get = "pub"]
  id: i32,
  /// Item type ID that defines the station's structure type.
  #[get = "pub"]
  item_type_id: i32,
  /// Maximum ship volume (m³) that may dock at this station.
  #[get = "pub"]
  max_dockable_ship_volume: f64,
  /// Display name of the station.
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  /// ISK cost to rent one office slot per month.
  #[get = "pub"]
  office_rental_cost: f64,
  /// Corporation or faction ID that owns this station, if any.
  #[get = "pub"]
  owner_id: Option<i32>,
  persisted: bool,
  /// X coordinate of the station's position in the solar system (metres).
  #[get = "pub"]
  position_x: f64,
  /// Y coordinate of the station's position in the solar system (metres).
  #[get = "pub"]
  position_y: f64,
  /// Z coordinate of the station's position in the solar system (metres).
  #[get = "pub"]
  position_z: f64,
  /// Race ID of the faction that owns this station, if any.
  #[get = "pub"]
  race_id: Option<i32>,
  /// Fraction of material value retained after reprocessing (0–1).
  #[get = "pub"]
  reprocessing_efficiency: f64,
  /// Fraction of reprocessed output taken by the station as a fee (0–1).
  #[get = "pub"]
  reprocessing_stations_take: f64,
  /// Services offered by this station (e.g. `"market"`, `"repair_facilities"`).
  #[getset(get = "pub", get_mut = "pub")]
  services: Vec<String>,
  /// ID of the solar system containing this station.
  #[get = "pub"]
  solar_system_id: i32,
}

impl Model {
  /// Creates a new unpersisted station with the given ID and name; all other fields default to zero/empty.
  pub fn new(id: i32, name: impl Into<String>) -> Self {
    Self {
      dirty: false,
      id,
      item_type_id: 0,
      max_dockable_ship_volume: 0.0,
      name: name.into(),
      office_rental_cost: 0.0,
      owner_id: None,
      persisted: false,
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      race_id: None,
      reprocessing_efficiency: 0.0,
      reprocessing_stations_take: 0.0,
      services: Vec::new(),
      solar_system_id: 0,
    }
  }

  /// Returns `true` if any field has been mutated since the model was loaded from the database.
  pub fn has_changes(&self) -> bool {
    self.dirty
  }

  /// Returns `true` if this model was loaded from, or successfully saved to, the database.
  pub fn is_persisted(&self) -> bool {
    self.persisted
  }

  /// Sets the item type ID and marks the model dirty if already persisted.
  pub fn set_item_type_id(&mut self, item_type_id: i32) -> &mut Self {
    self.item_type_id = item_type_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the maximum dockable ship volume and marks the model dirty if already persisted.
  pub fn set_max_dockable_ship_volume(&mut self, max_dockable_ship_volume: f64) -> &mut Self {
    self.max_dockable_ship_volume = max_dockable_ship_volume;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the station name and marks the model dirty if already persisted.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the office rental cost and marks the model dirty if already persisted.
  pub fn set_office_rental_cost(&mut self, office_rental_cost: f64) -> &mut Self {
    self.office_rental_cost = office_rental_cost;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the owner ID and marks the model dirty if already persisted.
  pub fn set_owner_id(&mut self, owner_id: Option<i32>) -> &mut Self {
    self.owner_id = owner_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the station's X/Y/Z position in the solar system and marks the model dirty if already persisted.
  pub fn set_position(&mut self, x: f64, y: f64, z: f64) -> &mut Self {
    self.position_x = x;
    self.position_y = y;
    self.position_z = z;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the race ID and marks the model dirty if already persisted.
  pub fn set_race_id(&mut self, race_id: Option<i32>) -> &mut Self {
    self.race_id = race_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the reprocessing efficiency fraction and marks the model dirty if already persisted.
  pub fn set_reprocessing_efficiency(&mut self, reprocessing_efficiency: f64) -> &mut Self {
    self.reprocessing_efficiency = reprocessing_efficiency;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the fraction of reprocessed output taken by the station and marks the model dirty if already persisted.
  pub fn set_reprocessing_stations_take(&mut self, reprocessing_stations_take: f64) -> &mut Self {
    self.reprocessing_stations_take = reprocessing_stations_take;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the solar system ID and marks the model dirty if already persisted.
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
