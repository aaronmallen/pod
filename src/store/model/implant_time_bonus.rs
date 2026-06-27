use sqlx::FromRow;

/// One `(type_id, attribute_id, value)` time-bonus row extracted from an item's `dogma_attributes` JSON blob,
/// where `value` is the raw SDE percent (negative for a reduction).
#[derive(Clone, Debug, FromRow, PartialEq)]
pub struct ImplantTimeBonus {
  pub attribute_id: i64,
  pub type_id: i64,
  pub value: f64,
}
