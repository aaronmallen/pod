use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::character::{Location, Online, Ship};

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub character_id: i64,
  #[getset(get_copy = "pub")]
  pub online: bool,
  #[getset(get_copy = "pub")]
  pub ship_item_id: Option<i64>,
  #[getset(get = "pub")]
  pub ship_name: Option<String>,
  #[getset(get_copy = "pub")]
  pub ship_type_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub solar_system_id: i64,
  #[getset(get_copy = "pub")]
  pub station_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub structure_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub synced_at: i64,
}

impl From<(i64, Online, Location, Option<Ship>, i64)> for Model {
  fn from((character_id, online, location, ship, synced_at): (i64, Online, Location, Option<Ship>, i64)) -> Self {
    Self {
      character_id,
      online: online.online,
      ship_item_id: ship.as_ref().map(|s| s.ship_item_id),
      ship_name: ship.as_ref().map(|s| s.ship_name.clone()),
      ship_type_id: ship.as_ref().map(|s| i64::from(s.ship_type_id)),
      solar_system_id: location.solar_system_id,
      station_id: location.station_id,
      structure_id: location.structure_id,
      synced_at,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_maps_ship_columns_when_a_ship_is_present() {
      let online = Online {
        online: true,
      };
      let location = Location {
        solar_system_id: 30_000_142,
        station_id: Some(60_003_760),
        structure_id: None,
      };
      let ship = Ship {
        ship_item_id: 1_000_000_016_991,
        ship_name: "My Rifter".to_owned(),
        ship_type_id: 587,
      };

      let model = Model::from((42, online, location, Some(ship), 1_700_000_000));

      assert_eq!(model.character_id(), 42);
      assert!(model.online());
      assert_eq!(model.solar_system_id(), 30_000_142);
      assert_eq!(model.station_id(), Some(60_003_760));
      assert_eq!(model.ship_item_id(), Some(1_000_000_016_991));
      assert_eq!(model.ship_name().as_deref(), Some("My Rifter"));
      assert_eq!(model.ship_type_id(), Some(587));
      assert_eq!(model.synced_at(), 1_700_000_000);
    }

    #[test]
    fn it_leaves_ship_columns_null_when_no_ship() {
      let online = Online {
        online: false,
      };
      let location = Location {
        solar_system_id: 30_000_142,
        station_id: None,
        structure_id: Some(1_021_000_000_000),
      };

      let model = Model::from((42, online, location, None, 1_700_000_000));

      assert!(!model.online());
      assert_eq!(model.structure_id(), Some(1_021_000_000_000));
      assert_eq!(model.ship_item_id(), None);
      assert_eq!(model.ship_name().as_deref(), None);
      assert_eq!(model.ship_type_id(), None);
    }
  }
}
