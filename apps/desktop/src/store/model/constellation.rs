use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::universe::Constellation;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
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
  #[getset(get_copy = "pub")]
  pub region_id: i64,
}

impl From<Constellation> for Model {
  fn from(constellation: Constellation) -> Self {
    Self {
      id: constellation.constellation_id,
      name: constellation.name,
      position_x: constellation.position.x,
      position_y: constellation.position.y,
      position_z: constellation.position.z,
      region_id: constellation.region_id,
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
    fn it_flattens_the_position() {
      let constellation = Constellation {
        constellation_id: 20_000_020,
        name: "Kimotoro".to_owned(),
        position: Position {
          x: 1.0,
          y: 2.0,
          z: 3.0,
        },
        region_id: 10_000_002,
        systems: vec![30_000_142],
      };

      let model = Model::from(constellation);

      assert_eq!(model.id(), 20_000_020);
      assert_eq!(model.position_x(), 1.0);
      assert_eq!(model.position_z(), 3.0);
      assert_eq!(model.region_id(), 10_000_002);
    }
  }
}
