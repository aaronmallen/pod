//! Domain model for an EVE Online item category.

use getset::{Getters, MutGetters};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// An item category, grouping related item groups under a common classification.
///
/// Tracks whether the category has unsaved changes (`dirty`) and whether it exists in the
/// database (`persisted`), enabling lightweight change detection before persistence.
#[derive(Clone, Debug, Deserialize, Getters, MutGetters, PartialEq, Serialize, Validate)]
pub struct Model {
  dirty: bool,
  #[get = "pub"]
  id: i32,
  #[getset(get = "pub", get_mut = "pub")]
  item_groups: Vec<crate::item_group::Model>,
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  persisted: bool,
  published: bool,
}

impl Model {
  /// Creates a new unpersisted item category with the given `id` and `name`.
  ///
  /// The category is published by default and has no associated item groups.
  pub fn new(id: i32, name: impl Into<String>) -> Self {
    Self {
      dirty: false,
      id,
      item_groups: Vec::new(),
      name: name.into(),
      persisted: false,
      published: true,
    }
  }

  /// Returns `true` if the model has unsaved field changes since it was last persisted.
  pub fn has_changes(&self) -> bool {
    self.dirty
  }

  /// Returns `true` if the model was loaded from or has been saved to the database.
  pub fn is_persisted(&self) -> bool {
    self.persisted
  }

  /// Returns `true` if the category is visible in-game.
  pub fn is_published(&self) -> bool {
    self.published
  }

  /// Marks the category as published, flagging it dirty when already persisted.
  pub fn publish(&mut self) -> &mut Self {
    self.published = true;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Updates the category name, flagging it dirty when already persisted.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Marks the category as unpublished, flagging it dirty when already persisted.
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
