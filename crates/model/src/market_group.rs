//! Domain model for EVE Online market groups.

use getset::{Getters, MutGetters};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// A market group used to organise item types in the in-game market browser.
#[derive(Clone, Debug, Deserialize, Getters, MutGetters, PartialEq, Serialize, Validate)]
pub struct Model {
  #[get = "pub"]
  description: Option<String>,
  dirty: bool,
  #[get = "pub"]
  id: i32,
  #[getset(get = "pub", get_mut = "pub")]
  item_types: Vec<crate::item_type::Model>,
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  #[get = "pub"]
  parent_market_group_id: Option<i32>,
  persisted: bool,
}

impl Model {
  /// Creates a new unpersisted market group with the given `id` and `name`.
  pub fn new(id: i32, name: impl Into<String>) -> Self {
    Self {
      description: None,
      dirty: false,
      id,
      item_types: Vec::new(),
      name: name.into(),
      parent_market_group_id: None,
      persisted: false,
    }
  }

  /// Returns `true` if any fields have been mutated since the model was last persisted.
  pub fn has_changes(&self) -> bool {
    self.dirty
  }

  /// Returns `true` if this model was loaded from or has been saved to the database.
  pub fn is_persisted(&self) -> bool {
    self.persisted
  }

  /// Sets the description, marking the model dirty if it has already been persisted.
  pub fn set_description(&mut self, description: Option<String>) -> &mut Self {
    self.description = description;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the name, marking the model dirty if it has already been persisted.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the parent market group ID, marking the model dirty if it has already been persisted.
  pub fn set_parent_market_group_id(&mut self, parent_market_group_id: Option<i32>) -> &mut Self {
    self.parent_market_group_id = parent_market_group_id;
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
