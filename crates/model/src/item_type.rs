//! Domain model for EVE Online item types (inventory types).

use getset::{Getters, MutGetters};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// A single dogma attribute value attached to an item type.
#[derive(Clone, Debug, Deserialize, Getters, PartialEq, Serialize)]
pub struct DogmaAttributeEntry {
  /// The numeric ID of the attribute.
  #[get = "pub"]
  attribute_id: i32,
  /// The attribute's value for this item type.
  #[get = "pub"]
  value: f64,
}

impl DogmaAttributeEntry {
  pub fn new(attribute_id: i32, value: f64) -> Self {
    Self {
      attribute_id,
      value,
    }
  }
}

/// A dogma effect associated with an item type.
#[derive(Clone, Debug, Deserialize, Eq, Getters, Hash, PartialEq, Serialize)]
pub struct DogmaEffectEntry {
  /// The numeric ID of the effect.
  #[get = "pub"]
  effect_id: i32,
  /// Whether this effect is the item's default active effect.
  #[get = "pub"]
  is_default: bool,
}

impl DogmaEffectEntry {
  pub fn new(effect_id: i32, is_default: bool) -> Self {
    Self {
      effect_id,
      is_default,
    }
  }
}

/// An item type (inventory type) with its physical and dogma properties.
///
/// Tracks whether the record has been saved to the database (`persisted`) and whether any
/// field has changed since the last save (`dirty`).
#[derive(Clone, Debug, Deserialize, Getters, MutGetters, PartialEq, Serialize, Validate)]
pub struct Model {
  /// Internal cargo capacity in m³, if applicable.
  #[get = "pub"]
  capacity: Option<f64>,
  /// The in-game description of the type.
  #[get = "pub"]
  description: String,
  dirty: bool,
  /// Dogma attributes associated with this type.
  #[getset(get = "pub", get_mut = "pub")]
  dogma_attributes: Vec<DogmaAttributeEntry>,
  /// Dogma effects associated with this type.
  #[getset(get = "pub", get_mut = "pub")]
  dogma_effects: Vec<DogmaEffectEntry>,
  /// The graphic resource ID, if any.
  #[get = "pub"]
  graphic_id: Option<i32>,
  /// The icon resource ID, if any.
  #[get = "pub"]
  icon_id: Option<i32>,
  /// The unique type ID.
  #[get = "pub"]
  id: i32,
  /// The item group this type belongs to.
  #[get = "pub"]
  item_group_id: i32,
  /// The market group this type belongs to, if any.
  #[get = "pub"]
  market_group_id: Option<i32>,
  /// Mass in kg, if applicable.
  #[get = "pub"]
  mass: Option<f64>,
  /// The display name of the type.
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  /// Volume when packaged in m³, if applicable.
  #[get = "pub"]
  packaged_volume: Option<f64>,
  persisted: bool,
  /// Number of units per stack/portion, if applicable.
  #[get = "pub"]
  portion_size: Option<i32>,
  /// Whether this item type is visible in the public game client.
  #[get = "pub"]
  published: bool,
  /// Collision/bounding radius in m, if applicable.
  #[get = "pub"]
  radius: Option<f64>,
  /// Volume in m³, if applicable.
  #[get = "pub"]
  volume: Option<f64>,
}

impl Model {
  /// Creates a new unpersisted item type with the given ID and name.
  pub fn new(id: i32, name: impl Into<String>) -> Self {
    Self {
      capacity: None,
      description: String::new(),
      dogma_attributes: Vec::new(),
      dogma_effects: Vec::new(),
      dirty: false,
      graphic_id: None,
      id,
      icon_id: None,
      item_group_id: 0,
      market_group_id: None,
      mass: None,
      name: name.into(),
      packaged_volume: None,
      persisted: false,
      portion_size: None,
      published: true,
      radius: None,
      volume: None,
    }
  }

  /// Returns `true` if any field has been mutated since the record was last persisted.
  pub fn has_changes(&self) -> bool {
    self.dirty
  }

  /// Returns `true` if this model was loaded from or saved to the database.
  pub fn is_persisted(&self) -> bool {
    self.persisted
  }

  /// Sets the cargo capacity in m³, marking the record dirty if already persisted.
  pub fn set_capacity(&mut self, capacity: Option<f64>) -> &mut Self {
    self.capacity = capacity;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the description, marking the record dirty if already persisted.
  pub fn set_description(&mut self, description: impl Into<String>) -> &mut Self {
    self.description = description.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the graphic ID, marking the record dirty if already persisted.
  pub fn set_graphic_id(&mut self, graphic_id: Option<i32>) -> &mut Self {
    self.graphic_id = graphic_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the icon ID, marking the record dirty if already persisted.
  pub fn set_icon_id(&mut self, icon_id: Option<i32>) -> &mut Self {
    self.icon_id = icon_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the item group ID, marking the record dirty if already persisted.
  pub fn set_item_group_id(&mut self, item_group_id: i32) -> &mut Self {
    self.item_group_id = item_group_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the market group ID, marking the record dirty if already persisted.
  pub fn set_market_group_id(&mut self, market_group_id: Option<i32>) -> &mut Self {
    self.market_group_id = market_group_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the mass in kg, marking the record dirty if already persisted.
  pub fn set_mass(&mut self, mass: Option<f64>) -> &mut Self {
    self.mass = mass;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the display name, marking the record dirty if already persisted.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the packaged volume in m³, marking the record dirty if already persisted.
  pub fn set_packaged_volume(&mut self, packaged_volume: Option<f64>) -> &mut Self {
    self.packaged_volume = packaged_volume;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the portion size, marking the record dirty if already persisted.
  pub fn set_portion_size(&mut self, portion_size: Option<i32>) -> &mut Self {
    self.portion_size = portion_size;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the bounding radius in m, marking the record dirty if already persisted.
  pub fn set_radius(&mut self, radius: Option<f64>) -> &mut Self {
    self.radius = radius;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the volume in m³, marking the record dirty if already persisted.
  pub fn set_volume(&mut self, volume: Option<f64>) -> &mut Self {
    self.volume = volume;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the published flag, marking the record dirty if already persisted.
  pub fn set_published(&mut self, published: bool) -> &mut Self {
    self.published = published;
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
