//! Character asset domain model.

use validator::Validate;

/// A character asset record. `item_id` is the primary key.
#[derive(Clone, Debug, Validate)]
pub struct Model {
  pub item_id: i64,
  pub character_id: i64,
  pub type_id: i32,
  pub location_id: i64,
  #[validate(length(min = 1))]
  pub location_type: String,
  #[validate(length(min = 1))]
  pub location_flag: String,
  pub quantity: i32,
  pub is_singleton: bool,
  pub is_blueprint_copy: Option<bool>,
}
