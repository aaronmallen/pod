//! Database entity for abyssal item records.

use pod_model::{AbyssalAttribute, AbyssalItemRecord};
use sea_orm::{FromJsonQueryResult, Set, prelude::*};
use serde::{Deserialize, Serialize};

/// A single rolled dogma attribute stored in the JSON column.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AttributeEntry {
  /// EVE dogma attribute ID.
  pub attribute_id: i32,
  /// Rolled value for this attribute.
  pub value: f64,
}

/// A list of attribute entries; serializes as a JSON array.
#[derive(Clone, Debug, Default, Deserialize, FromJsonQueryResult, PartialEq, Serialize)]
pub struct AttributeList(pub Vec<AttributeEntry>);

/// A persisted abyssal item record.
#[sea_orm::model]
#[derive(Clone, Debug, DeriveEntityModel, PartialEq)]
#[sea_orm(table_name = "abyssal_items")]
pub struct Model {
  /// The character that owns this item.
  pub character_id: i64,
  /// Rolled dogma attribute values, stored as JSON.
  pub dogma_attributes: AttributeList,
  /// The unique EVE item ID for this singleton.
  #[sea_orm(primary_key, auto_increment = false)]
  pub item_id: i64,
  /// MutaMarket estimated price in ISK.
  pub muta_price_isk: Option<f64>,
  /// Unix timestamp when the MutaMarket price was last fetched.
  pub muta_price_synced: Option<i64>,
  /// Type ID of the mutaplasmid used to create this item.
  pub mutator_type_id: i32,
  /// Type ID of the base (un-mutated) item.
  pub source_type_id: i32,
  /// Unix timestamp when dogma data was last fetched.
  pub synced_at: i64,
  /// The resulting abyssal type ID.
  pub type_id: i32,
}

/// Default active-model behaviour with no custom hooks.
impl ActiveModelBehavior for ActiveModel {}

impl From<AttributeEntry> for AbyssalAttribute {
  fn from(entry: AttributeEntry) -> Self {
    AbyssalAttribute::new(entry.attribute_id, entry.value)
  }
}

impl From<AbyssalAttribute> for AttributeEntry {
  fn from(attr: AbyssalAttribute) -> Self {
    Self {
      attribute_id: *attr.attribute_id(),
      value: *attr.value(),
    }
  }
}

impl From<Model> for AbyssalItemRecord {
  fn from(entity: Model) -> Self {
    let attrs = entity.dogma_attributes.0.into_iter().map(Into::into).collect();
    let mut record = AbyssalItemRecord::new(
      entity.item_id,
      entity.character_id,
      entity.type_id,
      entity.source_type_id,
      entity.mutator_type_id,
      attrs,
      entity.synced_at,
    );
    record.set_muta_price(entity.muta_price_isk, entity.muta_price_synced.unwrap_or(0));
    record
  }
}

impl From<AbyssalItemRecord> for ActiveModel {
  fn from(record: AbyssalItemRecord) -> Self {
    let attrs = AttributeList(record.dogma_attributes().iter().cloned().map(Into::into).collect());
    Self {
      character_id: Set(*record.character_id()),
      dogma_attributes: Set(attrs),
      item_id: Set(*record.item_id()),
      muta_price_isk: Set(*record.muta_price_isk()),
      muta_price_synced: Set(*record.muta_price_synced()),
      mutator_type_id: Set(*record.mutator_type_id()),
      source_type_id: Set(*record.source_type_id()),
      synced_at: Set(*record.synced_at()),
      type_id: Set(*record.type_id()),
    }
  }
}
