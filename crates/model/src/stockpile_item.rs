//! Domain model for stockpile items.

use getset::Getters;
use validator::Validate;

/// A single item requirement within a stockpile.
#[derive(Clone, Debug, Getters, PartialEq, Validate)]
pub struct StockpileItem {
  /// Unique identifier for this stockpile item.
  #[get = "pub"]
  id: i64,
  /// FK to the owning stockpile.
  #[get = "pub"]
  stockpile_id: i64,
  /// Desired quantity to keep stocked.
  #[get = "pub"]
  target_quantity: i32,
  /// EVE type ID of the item.
  #[get = "pub"]
  type_id: i32,
}

impl StockpileItem {
  /// Creates a new stockpile item with the given identifiers and quantity.
  pub fn new(id: i64, stockpile_id: i64, type_id: i32, target_quantity: i32) -> Self {
    Self {
      id,
      stockpile_id,
      target_quantity,
      type_id,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod stockpile_item {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_stores_fields() {
      let item = StockpileItem::new(1, 10, 587, 50);

      assert_eq!(*item.id(), 1);
      assert_eq!(*item.stockpile_id(), 10);
      assert_eq!(*item.type_id(), 587);
      assert_eq!(*item.target_quantity(), 50);
    }
  }
}
