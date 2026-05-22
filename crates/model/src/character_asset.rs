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

#[cfg(test)]
mod tests {
  use validator::Validate;

  use super::*;

  fn make_asset() -> Model {
    Model {
      item_id: 1001,
      character_id: 90_000_001,
      type_id: 587,
      location_id: 60_003_760,
      location_type: "station".into(),
      location_flag: "Hangar".into(),
      quantity: 1,
      is_singleton: false,
      is_blueprint_copy: None,
    }
  }

  mod validate {
    use super::*;

    #[test]
    fn it_passes_for_valid_asset() {
      let asset = make_asset();
      assert!(asset.validate().is_ok());
    }

    #[test]
    fn it_fails_when_location_type_is_empty() {
      let mut asset = make_asset();
      asset.location_type = String::new();
      assert!(asset.validate().is_err());
    }

    #[test]
    fn it_fails_when_location_flag_is_empty() {
      let mut asset = make_asset();
      asset.location_flag = String::new();
      assert!(asset.validate().is_err());
    }
  }
}
