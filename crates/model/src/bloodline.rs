//! Domain model for EVE Online bloodlines.

use getset::Getters;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// A bloodline, representing a character ancestry option within a race.
///
/// Tracks both the static EVE data (attribute scores, associated corporation and
/// starter ship) and dirty/persisted state for change detection before database writes.
#[derive(Clone, Debug, Deserialize, Getters, PartialEq, Serialize, Validate)]
pub struct Model {
  /// Base charisma attribute bonus granted to characters of this bloodline.
  #[get = "pub"]
  charisma: i32,
  /// ID of the NPC corporation associated with this bloodline.
  #[get = "pub"]
  corporation_id: i32,
  /// Human-readable description of the bloodline.
  #[get = "pub"]
  description: String,
  dirty: bool,
  /// Unique bloodline identifier.
  #[get = "pub"]
  id: i32,
  /// Base intelligence attribute bonus granted to characters of this bloodline.
  #[get = "pub"]
  intelligence: i32,
  /// Base memory attribute bonus granted to characters of this bloodline.
  #[get = "pub"]
  memory: i32,
  /// Display name of the bloodline.
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
  /// Base perception attribute bonus granted to characters of this bloodline.
  #[get = "pub"]
  perception: i32,
  persisted: bool,
  /// Foreign key referencing the parent race.
  #[get = "pub"]
  race_id: i32,
  /// Eagerly loaded race associated with this bloodline, if available.
  #[get = "pub"]
  race: Option<crate::race::Model>,
  /// Item type ID of the starter ship granted to characters of this bloodline.
  #[get = "pub"]
  ship_item_type_id: i32,
  /// Eagerly loaded item type for the starter ship, if available.
  #[get = "pub"]
  ship_item_type: Option<crate::item_type::Model>,
  /// Base willpower attribute bonus granted to characters of this bloodline.
  #[get = "pub"]
  will_power: i32,
}

impl Model {
  /// Creates a new unpersisted bloodline with all attribute scores set to zero.
  pub fn new(id: i32, name: impl Into<String>) -> Self {
    Self {
      charisma: 0,
      corporation_id: 0,
      description: String::new(),
      dirty: false,
      id,
      intelligence: 0,
      memory: 0,
      name: name.into(),
      perception: 0,
      persisted: false,
      race_id: 0,
      race: None,
      ship_item_type_id: 0,
      ship_item_type: None,
      will_power: 0,
    }
  }

  /// Returns `true` if any field has been modified since the model was last persisted.
  pub fn has_changes(&self) -> bool {
    self.dirty
  }

  /// Returns `true` if this model was loaded from the database.
  pub fn is_persisted(&self) -> bool {
    self.persisted
  }

  /// Sets the charisma attribute score, marking the model dirty if already persisted.
  pub fn set_charisma(&mut self, charisma: i32) -> &mut Self {
    self.charisma = charisma;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the ID of the NPC corporation associated with this bloodline.
  pub fn set_corporation_id(&mut self, corporation_id: i32) -> &mut Self {
    self.corporation_id = corporation_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the lore description for this bloodline.
  pub fn set_description(&mut self, description: impl Into<String>) -> &mut Self {
    self.description = description.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the intelligence attribute score, marking the model dirty if already persisted.
  pub fn set_intelligence(&mut self, intelligence: i32) -> &mut Self {
    self.intelligence = intelligence;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the memory attribute score, marking the model dirty if already persisted.
  pub fn set_memory(&mut self, memory: i32) -> &mut Self {
    self.memory = memory;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the display name of this bloodline.
  pub fn set_name(&mut self, name: impl Into<String>) -> &mut Self {
    self.name = name.into();
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the perception attribute score, marking the model dirty if already persisted.
  pub fn set_perception(&mut self, perception: i32) -> &mut Self {
    self.perception = perception;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the ID of the race this bloodline belongs to.
  pub fn set_race_id(&mut self, race_id: i32) -> &mut Self {
    self.race_id = race_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the eagerly loaded race associated with this bloodline.
  pub fn set_race(&mut self, race: Option<crate::race::Model>) -> &mut Self {
    self.race = race;
    self
  }

  /// Sets the item type ID of the starter ship granted to characters of this bloodline.
  pub fn set_ship_item_type_id(&mut self, ship_item_type_id: i32) -> &mut Self {
    self.ship_item_type_id = ship_item_type_id;
    if self.persisted {
      self.dirty = true;
    }
    self
  }

  /// Sets the eagerly loaded item type for the starter ship associated with this bloodline.
  pub fn set_ship_item_type(&mut self, ship_item_type: Option<crate::item_type::Model>) -> &mut Self {
    self.ship_item_type = ship_item_type;
    self
  }

  /// Sets the willpower attribute score, marking the model dirty if already persisted.
  pub fn set_will_power(&mut self, will_power: i32) -> &mut Self {
    self.will_power = will_power;
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

  fn make_bloodline() -> Model {
    let mut b = Model::new(1, "Achura");
    b.set_race_id(1)
      .set_corporation_id(1000033)
      .set_ship_item_type_id(601)
      .set_description("Achura bloodline description")
      .set_charisma(6)
      .set_intelligence(10)
      .set_memory(9)
      .set_perception(9)
      .set_will_power(8);
    b
  }

  mod has_changes {
    use super::*;

    #[test]
    fn it_returns_false_before_persist() {
      let mut b = make_bloodline();
      b.set_charisma(7);
      assert!(!b.has_changes());
    }

    #[test]
    fn it_returns_true_after_persist_and_mutation() {
      let mut b = make_bloodline();
      b.mark_persisted();
      b.set_charisma(7);
      assert!(b.has_changes());
    }
  }

  mod mark_persisted {
    use super::*;

    #[test]
    fn it_marks_model_as_persisted_without_dirtying() {
      let mut b = make_bloodline();
      b.mark_persisted();
      assert!(b.is_persisted());
      assert!(!b.has_changes());
    }
  }

  mod set_corporation_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut b = make_bloodline();
      b.mark_persisted();
      b.set_corporation_id(1_000_001);
      assert!(b.has_changes());
    }
  }

  mod set_description {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut b = make_bloodline();
      b.mark_persisted();
      b.set_description("Updated description.");
      assert!(b.has_changes());
    }
  }

  mod set_intelligence {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut b = make_bloodline();
      b.mark_persisted();
      b.set_intelligence(11);
      assert!(b.has_changes());
    }
  }

  mod set_memory {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut b = make_bloodline();
      b.mark_persisted();
      b.set_memory(10);
      assert!(b.has_changes());
    }
  }

  mod set_name {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut b = make_bloodline();
      b.mark_persisted();
      b.set_name("New Name");
      assert!(b.has_changes());
    }
  }

  mod set_perception {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut b = make_bloodline();
      b.mark_persisted();
      b.set_perception(10);
      assert!(b.has_changes());
    }
  }

  mod set_race_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut b = make_bloodline();
      b.mark_persisted();
      b.set_race_id(2);
      assert!(b.has_changes());
    }
  }

  mod set_ship_item_type_id {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut b = make_bloodline();
      b.mark_persisted();
      b.set_ship_item_type_id(602);
      assert!(b.has_changes());
    }
  }

  mod set_will_power {
    use super::*;

    #[test]
    fn it_marks_dirty_when_persisted() {
      let mut b = make_bloodline();
      b.mark_persisted();
      b.set_will_power(9);
      assert!(b.has_changes());
    }
  }
}
