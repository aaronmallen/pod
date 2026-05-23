//! Character asset domain model.

use validator::Validate;

/// A character asset record. `item_id` is the primary key.
#[derive(Clone, Debug, Default, Validate)]
pub struct Model {
  pub character_id: i64,
  /// True when this row represents the character's currently-boarded ship.
  /// Ships in space are absent from the ESI assets endpoint and are injected
  /// as synthetic rows by the background sync job.
  pub is_active_ship: bool,
  pub is_blueprint_copy: Option<bool>,
  pub is_singleton: bool,
  pub item_id: i64,
  #[validate(length(min = 1))]
  pub location_flag: String,
  pub location_id: i64,
  #[validate(length(min = 1))]
  pub location_type: String,
  pub quantity: i32,
  /// Display name of the ship when `is_active_ship` is true; `None` otherwise.
  pub ship_name: Option<String>,
  pub type_id: i32,
}

#[cfg(test)]
mod tests {
  use validator::Validate;

  use super::*;

  fn make_asset() -> Model {
    Model {
      character_id: 90_000_001,
      is_active_ship: false,
      is_blueprint_copy: None,
      is_singleton: false,
      item_id: 1001,
      location_flag: "Hangar".into(),
      location_id: 60_003_760,
      location_type: "station".into(),
      quantity: 1,
      ship_name: None,
      type_id: 587,
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
