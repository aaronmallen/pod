//! Dogma attribute newtypes for JSON serialization in item_types.

use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};

/// A single dogma attribute value entry on an item type.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Entry {
  /// The EVE dogma attribute identifier.
  pub attribute_id: i32,
  /// The numeric value assigned to this attribute.
  pub value: f64,
}

/// A list of dogma attribute entries; serializes as a JSON array.
#[derive(Clone, Debug, Default, Deserialize, FromJsonQueryResult, PartialEq, Serialize)]
pub struct List(pub Vec<Entry>);
