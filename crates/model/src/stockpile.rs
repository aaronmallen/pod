//! Domain model for stockpiles.

use getset::{Getters, MutGetters};
use validator::Validate;

/// A named stockpile that tracks desired item quantities for a character or location.
#[derive(Clone, Debug, Getters, MutGetters, PartialEq, Validate)]
pub struct Stockpile {
  /// Optional EVE character ID this stockpile is scoped to; `None` means all characters.
  #[get = "pub"]
  character_id: Option<i64>,
  /// Unique identifier for this stockpile.
  #[get = "pub"]
  id: i64,
  /// Item requirements belonging to this stockpile.
  #[getset(get = "pub", get_mut = "pub")]
  items: Vec<super::StockpileItem>,
  /// Optional EVE location ID this stockpile is scoped to; `None` means all locations.
  #[get = "pub"]
  location_id: Option<i64>,
  /// Display name of the stockpile.
  #[get = "pub"]
  #[validate(length(min = 1))]
  name: String,
}

impl Stockpile {
  /// Creates a new stockpile with the given ID and name.
  pub fn new(id: i64, name: impl Into<String>) -> Self {
    Self {
      character_id: None,
      id,
      items: vec![],
      location_id: None,
      name: name.into(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod stockpile {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_stores_fields() {
      let s = Stockpile::new(1, "My Stockpile");

      assert_eq!(*s.id(), 1);
      assert_eq!(s.name(), "My Stockpile");
      assert!(s.items().is_empty());
      assert!(s.character_id().is_none());
      assert!(s.location_id().is_none());
    }

    #[test]
    fn it_initializes_items_to_empty_vec() {
      let s = Stockpile::new(42, "Empty");

      assert_eq!(s.items(), &vec![]);
    }
  }
}
