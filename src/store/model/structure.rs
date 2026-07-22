use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::universe::Structure;

#[derive(Clone, CopyGetters, Debug, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub owner_id: i64,
  #[getset(get_copy = "pub")]
  pub position_x: Option<f64>,
  #[getset(get_copy = "pub")]
  pub position_y: Option<f64>,
  #[getset(get_copy = "pub")]
  pub position_z: Option<f64>,
  #[getset(get_copy = "pub")]
  pub solar_system_id: i64,
  #[getset(get_copy = "pub")]
  pub type_id: Option<i64>,
}

impl From<(i64, Structure)> for Model {
  fn from((id, structure): (i64, Structure)) -> Self {
    Self {
      id,
      name: structure.name,
      owner_id: structure.owner_id,
      position_x: structure.position.as_ref().map(|p| p.x),
      position_y: structure.position.as_ref().map(|p| p.y),
      position_z: structure.position.as_ref().map(|p| p.z),
      solar_system_id: structure.solar_system_id,
      type_id: structure.type_id.map(i64::from),
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
    fn it_applies_path_id_and_flattens_present_position() {
      let structure = Structure {
        name: "A Player Structure".to_owned(),
        owner_id: 98_000_001,
        position: Some(Position {
          x: 1.0,
          y: 2.0,
          z: 3.0,
        }),
        solar_system_id: 30_000_142,
        type_id: Some(35_833),
      };

      let model = Model::from((1_021_000_000_000_i64, structure));

      assert_eq!(model.id(), 1_021_000_000_000_i64);
      assert_eq!(model.position_x(), Some(1.0));
      assert_eq!(model.position_z(), Some(3.0));
      assert_eq!(model.type_id(), Some(35_833));
    }

    #[test]
    fn it_maps_absent_position_to_none() {
      let structure = Structure {
        name: "Hidden Structure".to_owned(),
        owner_id: 98_000_001,
        position: None,
        solar_system_id: 30_000_142,
        type_id: None,
      };

      let model = Model::from((1_021_000_000_001_i64, structure));

      assert_eq!(model.position_x(), None);
      assert_eq!(model.position_y(), None);
      assert_eq!(model.position_z(), None);
      assert_eq!(model.type_id(), None);
    }
  }
}
