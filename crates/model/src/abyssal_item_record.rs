//! Domain model for a stored abyssal (mutated) item record.

use getset::Getters;
use serde::{Deserialize, Serialize};

/// A single rolled dogma attribute value on an abyssal item.
#[derive(Clone, Debug, Deserialize, Getters, PartialEq, Serialize)]
pub struct AbyssalAttribute {
  /// The EVE dogma attribute identifier.
  #[get = "pub"]
  attribute_id: i32,
  /// The rolled value for this attribute on this specific item.
  #[get = "pub"]
  value: f64,
}

impl AbyssalAttribute {
  /// Creates a new `AbyssalAttribute`.
  pub fn new(attribute_id: i32, value: f64) -> Self {
    Self {
      attribute_id,
      value,
    }
  }
}

/// A persisted abyssal (mutated) item record loaded from the DB.
#[derive(Clone, Debug, Deserialize, Getters, PartialEq, Serialize)]
pub struct AbyssalItemRecord {
  /// The character that owns this item.
  #[get = "pub"]
  character_id: i64,
  /// The rolled dogma attribute values for this item.
  #[get = "pub"]
  dogma_attributes: Vec<AbyssalAttribute>,
  /// The unique EVE item ID for this singleton.
  #[get = "pub"]
  item_id: i64,
  /// Estimated MutaMarket price in ISK, if available.
  #[get = "pub"]
  muta_price_isk: Option<f64>,
  /// Unix timestamp (seconds) when the MutaMarket price was last fetched.
  #[get = "pub"]
  muta_price_synced: Option<i64>,
  /// Type ID of the mutaplasmid used to create this item.
  #[get = "pub"]
  mutator_type_id: i32,
  /// Type ID of the base (un-mutated) item used as input.
  #[get = "pub"]
  source_type_id: i32,
  /// Unix timestamp (seconds) when the item's dogma data was last fetched.
  #[get = "pub"]
  synced_at: i64,
  /// The resulting abyssal type ID (the mutated type).
  #[get = "pub"]
  type_id: i32,
}

impl AbyssalItemRecord {
  /// Creates a new `AbyssalItemRecord`.
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    item_id: i64,
    character_id: i64,
    type_id: i32,
    source_type_id: i32,
    mutator_type_id: i32,
    dogma_attributes: Vec<AbyssalAttribute>,
    synced_at: i64,
  ) -> Self {
    Self {
      character_id,
      dogma_attributes,
      item_id,
      muta_price_isk: None,
      muta_price_synced: None,
      mutator_type_id,
      source_type_id,
      synced_at,
      type_id,
    }
  }

  /// Sets the MutaMarket price and sync timestamp.
  pub fn set_muta_price(&mut self, price_isk: Option<f64>, synced_at: i64) -> &mut Self {
    self.muta_price_isk = price_isk;
    self.muta_price_synced = Some(synced_at);
    self
  }
}
