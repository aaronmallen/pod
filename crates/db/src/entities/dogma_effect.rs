//! Dogma effect newtypes for JSON serialization in item_types.

use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};

/// A single dogma effect entry on an item type.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Entry {
  /// The unique identifier of the dogma effect.
  pub effect_id: i32,
  /// Whether this effect is the item's default active effect.
  pub is_default: bool,
}

/// A list of dogma effect entries; serializes as a JSON array.
#[derive(Clone, Debug, Default, Deserialize, FromJsonQueryResult, PartialEq, Serialize)]
pub struct List(pub Vec<Entry>);
