use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::universe::Station;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get_copy = "pub")]
  pub max_dockable_ship_volume: f64,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub office_rental_cost: f64,
  #[getset(get_copy = "pub")]
  pub owner: Option<i64>,
  #[getset(get_copy = "pub")]
  pub position_x: f64,
  #[getset(get_copy = "pub")]
  pub position_y: f64,
  #[getset(get_copy = "pub")]
  pub position_z: f64,
  #[getset(get_copy = "pub")]
  pub race_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub reprocessing_efficiency: f64,
  #[getset(get_copy = "pub")]
  pub reprocessing_stations_take: f64,
  #[getset(get = "pub")]
  pub services: String,
  #[getset(get_copy = "pub")]
  pub system_id: i64,
  #[getset(get_copy = "pub")]
  pub type_id: i64,
}

impl From<Station> for Model {
  fn from(station: Station) -> Self {
    Self {
      id: station.station_id,
      max_dockable_ship_volume: station.max_dockable_ship_volume,
      name: station.name,
      office_rental_cost: station.office_rental_cost,
      owner: station.owner,
      position_x: station.position.x,
      position_y: station.position.y,
      position_z: station.position.z,
      race_id: station.race_id.map(i64::from),
      reprocessing_efficiency: station.reprocessing_efficiency,
      reprocessing_stations_take: station.reprocessing_stations_take,
      services: station.services.join(","),
      system_id: station.system_id,
      type_id: i64::from(station.type_id),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::clients::esi::models::universe::Position;

    fn make_station() -> Station {
      Station {
        max_dockable_ship_volume: 50_000_000.0,
        name: "Jita IV - Moon 4".to_owned(),
        office_rental_cost: 10_000.0,
        owner: None,
        position: Position {
          x: 1.0,
          y: 2.0,
          z: 3.0,
        },
        race_id: None,
        reprocessing_efficiency: 0.5,
        reprocessing_stations_take: 0.05,
        services: vec!["market".to_owned(), "repair-facilities".to_owned()],
        station_id: 60_003_760,
        system_id: 30_000_142,
        type_id: 52678,
      }
    }

    #[test]
    fn it_joins_services_and_widens_type_id() {
      let model = Model::from(make_station());

      assert_eq!(model.id(), 60_003_760);
      assert_eq!(model.services(), "market,repair-facilities");
      assert_eq!(model.type_id(), 52678);
    }

    #[test]
    fn it_widens_optional_race_id_when_present() {
      let mut station = make_station();
      station.owner = Some(1_000_035);
      station.race_id = Some(1);

      let model = Model::from(station);

      assert_eq!(model.owner(), Some(1_000_035));
      assert_eq!(model.race_id(), Some(1));
    }
  }
}
