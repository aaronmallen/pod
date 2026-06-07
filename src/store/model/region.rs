use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::universe::Region;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get = "pub")]
  pub description: Option<String>,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub name: String,
}

impl From<Region> for Model {
  fn from(region: Region) -> Self {
    Self {
      description: region.description,
      id: region.region_id,
      name: region.name,
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
    fn it_maps_the_region_id_and_optional_description() {
      let region = Region {
        constellations: vec![20_000_001, 20_000_002],
        description: Some("The Forge.".to_owned()),
        name: "The Forge".to_owned(),
        region_id: 10_000_002,
      };

      let model = Model::from(region);

      assert_eq!(model.id(), 10_000_002);
      assert_eq!(model.name(), "The Forge");
      assert_eq!(model.description().as_deref(), Some("The Forge."));
    }
  }
}
