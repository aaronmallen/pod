//! Corporation asset domain model.

use validator::Validate;

/// A corporation asset record. `item_id` is the primary key.
#[derive(Clone, Debug, Default, Validate)]
pub struct Model {
  pub corporation_id: i64,
  pub is_blueprint_copy: Option<bool>,
  pub is_singleton: bool,
  pub item_id: i64,
  #[validate(length(min = 1))]
  pub location_flag: String,
  pub location_id: i64,
  #[validate(length(min = 1))]
  pub location_type: String,
  pub quantity: i32,
  pub type_id: i32,
}
