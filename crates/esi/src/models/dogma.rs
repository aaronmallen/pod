//! Dogma ESI response models.

use serde::{Deserialize, Serialize};

/// A dogma attribute definition.
#[derive(Debug, Deserialize, Serialize)]
pub struct DogmaAttribute {
  pub attribute_id: i32,
  pub default_value: Option<f64>,
  pub description: Option<String>,
  pub display_name: Option<String>,
  pub high_is_good: Option<bool>,
  pub icon_id: Option<i32>,
  pub name: Option<String>,
  pub published: Option<bool>,
  pub stackable: Option<bool>,
  pub unit_id: Option<i32>,
}

/// A dogma effect definition.
#[derive(Debug, Deserialize, Serialize)]
pub struct DogmaEffect {
  pub description: Option<String>,
  pub disallow_auto_repeat: Option<bool>,
  pub discharge_attribute_id: Option<i32>,
  pub display_name: Option<String>,
  pub duration_attribute_id: Option<i32>,
  pub effect_category: Option<i32>,
  pub effect_id: i32,
  pub electronic_chance: Option<bool>,
  pub falloff_attribute_id: Option<i32>,
  pub icon_id: Option<i32>,
  pub is_assistance: Option<bool>,
  pub is_offensive: Option<bool>,
  pub is_warp_safe: Option<bool>,
  pub modifiers: Option<Vec<serde_json::Value>>,
  pub name: Option<String>,
  pub post_expression: Option<i32>,
  pub pre_expression: Option<i32>,
  pub published: Option<bool>,
  pub range_attribute_id: Option<i32>,
  pub range_chance: Option<bool>,
  pub tracking_speed_attribute_id: Option<i32>,
}

/// A single dogma attribute value on a dynamic item.
#[derive(Debug, Deserialize, Serialize)]
pub struct DogmaAttrValue {
  pub attribute_id: i32,
  pub value: f64,
}

/// A single dogma effect entry on a dynamic item.
#[derive(Debug, Deserialize, Serialize)]
pub struct DogmaEffectValue {
  pub effect_id: i32,
  pub is_default: bool,
}

/// A dynamically mutated item.
#[derive(Debug, Deserialize, Serialize)]
pub struct DynamicItem {
  pub created_by: i64,
  pub dogma_attributes: Vec<DogmaAttrValue>,
  pub dogma_effects: Vec<DogmaEffectValue>,
  pub mutator_type_id: i32,
  pub source_type_id: i32,
}
