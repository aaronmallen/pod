//! Domain models for character clone management.

use getset::Getters;

/// A single implant installed in a clone.
#[derive(Clone, Debug, Getters, PartialEq)]
pub struct CloneImplant {
  /// Bonus granted by this implant (e.g., "+3 Perception").
  #[get = "pub"]
  attribute_bonus: String,
  /// Human-readable implant name.
  #[get = "pub"]
  name: String,
  /// Implant slot number (1–10).
  #[get = "pub"]
  slot: u8,
  /// EVE type ID for the implant item.
  #[get = "pub"]
  type_id: i32,
}

impl CloneImplant {
  /// Creates a new implant entry.
  pub fn new(slot: u8, type_id: i32, name: impl Into<String>, attribute_bonus: impl Into<String>) -> Self {
    Self {
      attribute_bonus: attribute_bonus.into(),
      name: name.into(),
      slot,
      type_id,
    }
  }

  /// Sets the attribute bonus description.
  pub fn set_attribute_bonus(&mut self, attribute_bonus: impl Into<String>) -> &mut Self {
    self.attribute_bonus = attribute_bonus.into();
    self
  }

  /// Sets the implant name.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    self
  }

  /// Sets the implant slot number.
  pub fn set_slot(&mut self, slot: u8) -> &mut Self {
    self.slot = slot;
    self
  }

  /// Sets the EVE type ID.
  pub fn set_type_id(&mut self, type_id: i32) -> &mut Self {
    self.type_id = type_id;
    self
  }
}

/// A character clone (active implant set or a stored jump clone).
#[derive(Clone, Debug, Getters, PartialEq)]
pub struct Clone {
  /// Implants installed in this clone.
  #[get = "pub"]
  implants: Vec<CloneImplant>,
  /// ISO-8601 timestamp of when the clone was installed.
  #[get = "pub"]
  installed_at: Option<String>,
  /// Whether this is the character's currently active clone.
  #[get = "pub"]
  is_active: bool,
  /// Optional user-assigned name for jump clones.
  #[get = "pub"]
  name: String,
  /// Resolved name of the region the clone is located in.
  #[get = "pub"]
  region_name: String,
  /// Resolved name of the station or structure the clone is docked at.
  #[get = "pub"]
  station_name: String,
  /// EVE solar system ID where the clone is located.
  #[get = "pub"]
  system_id: i64,
}

impl Clone {
  /// Creates a new clone entry.
  pub fn new(
    name: impl Into<String>,
    station_name: impl Into<String>,
    system_id: i64,
    region_name: impl Into<String>,
  ) -> Self {
    Self {
      implants: Vec::new(),
      installed_at: None,
      is_active: false,
      name: name.into(),
      region_name: region_name.into(),
      station_name: station_name.into(),
      system_id,
    }
  }

  /// Sets the list of implants installed in this clone.
  pub fn set_implants(&mut self, implants: Vec<CloneImplant>) -> &mut Self {
    self.implants = implants;
    self
  }

  /// Sets the installation timestamp.
  pub fn set_installed_at(&mut self, installed_at: Option<String>) -> &mut Self {
    self.installed_at = installed_at;
    self
  }

  /// Sets whether this is the active clone.
  pub fn set_is_active(&mut self, is_active: bool) -> &mut Self {
    self.is_active = is_active;
    self
  }

  /// Sets the clone name.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    self
  }

  /// Sets the region name.
  pub fn set_region_name(&mut self, region_name: impl Into<String>) -> &mut Self {
    self.region_name = region_name.into();
    self
  }

  /// Sets the station name.
  pub fn set_station_name(&mut self, station_name: impl Into<String>) -> &mut Self {
    self.station_name = station_name.into();
    self
  }

  /// Sets the solar system ID.
  pub fn set_system_id(&mut self, system_id: i64) -> &mut Self {
    self.system_id = system_id;
    self
  }
}
