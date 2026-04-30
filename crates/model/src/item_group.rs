//! Domain model for EVE Online item groups.

use getset::{Getters, MutGetters};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// An EVE Online item group, grouping related item types within a category.
///
/// Tracks whether the record has been saved to the database (`persisted`) and
/// whether any field has changed since the last save (`dirty`).
#[derive(Clone, Debug, Deserialize, Getters, MutGetters, PartialEq, Serialize, Validate)]
pub struct Model {
  dirty: bool,
  #[get = "pub"]
  id: i32,
  #[get = "pub"]
  item_category_id: i32,
  #[getset(get = "pub", get_mut = "pub")]
  item_types: Vec<crate::item_type::Model>,
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  persisted: bool,
  published: bool,
}

impl Model {
  /// Creates a new unpersisted item group, defaulting to published with no item types.
  pub fn new(id: i32, item_category_id: i32, name: impl Into<String>) -> Self {
    Self {
      dirty: false,
      id,
      item_category_id,
      item_types: Vec::new(),
      name: name.into(),
      persisted: false,
      published: true,
    }
  }

  /// Returns `true` if any field has been modified since the record was last persisted.
  pub fn has_changes(&self) -> bool {
    self.dirty
  }

  /// Returns `true` if this record exists in the database.
  pub fn is_persisted(&self) -> bool {
    self.persisted
  }

  /// Returns `true` if this item group is published and visible in the game.
  pub fn is_published(&self) -> bool {
    self.published
  }

  /// Marks the group as published, flagging the record as dirty if already persisted.
  pub fn publish(&mut self) -> &mut Self {
    self.published = true;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Updates the item category, flagging the record as dirty if already persisted.
  pub fn set_item_category_id(&mut self, item_category_id: i32) -> &mut Self {
    self.item_category_id = item_category_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Updates the group name, flagging the record as dirty if already persisted.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Marks the group as unpublished, flagging the record as dirty if already persisted.
  pub fn unpublish(&mut self) -> &mut Self {
    self.published = false;
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
