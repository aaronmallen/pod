use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::universe::SolarSystem;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub constellation_id: i64,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub position_x: f64,
  #[getset(get_copy = "pub")]
  pub position_y: f64,
  #[getset(get_copy = "pub")]
  pub position_z: f64,
  #[getset(get = "pub")]
  pub security_class: Option<String>,
  #[getset(get_copy = "pub")]
  pub security_status: f64,
  #[getset(get_copy = "pub")]
  pub star_id: Option<i64>,
}

impl From<SolarSystem> for Model {
  fn from(system: SolarSystem) -> Self {
    Self {
      constellation_id: system.constellation_id,
      id: system.system_id,
      name: system.name,
      position_x: system.position.x,
      position_y: system.position.y,
      position_z: system.position.z,
      security_class: system.security_class,
      security_status: system.security_status,
      star_id: system.star_id,
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

    #[test]
    fn it_flattens_position_and_keeps_optionals() {
      let system = SolarSystem {
        constellation_id: 20_000_020,
        name: "Jita".to_owned(),
        position: Position {
          x: 1.0,
          y: 2.0,
          z: 3.0,
        },
        security_class: Some("B".to_owned()),
        security_status: 0.946,
        star_id: Some(40_000_001),
        stargates: None,
        stations: None,
        system_id: 30_000_142,
      };

      let model = Model::from(system);

      assert_eq!(model.id(), 30_000_142);
      assert_eq!(model.position_y(), 2.0);
      assert_eq!(model.security_class().as_deref(), Some("B"));
      assert_eq!(model.star_id(), Some(40_000_001));
    }
  }
}
