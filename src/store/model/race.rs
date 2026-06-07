use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::races::Race;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub alliance_id: i64,
  #[getset(get = "pub")]
  pub description: String,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get = "pub")]
  pub name: String,
}

impl Model {
  pub fn new(id: i64, alliance_id: i64, description: impl Into<String>, name: impl Into<String>) -> Self {
    Self {
      alliance_id,
      description: description.into(),
      id,
      name: name.into(),
    }
  }
}

impl From<Race> for Model {
  fn from(race: Race) -> Self {
    Self::new(i64::from(race.race_id), race.alliance_id, race.description, race.name)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_widens_the_race_id() {
      let race = Race {
        alliance_id: 500_001,
        description: "Founded on patriotism.".to_owned(),
        name: "Caldari".to_owned(),
        race_id: 1,
      };

      let model = Model::from(race);

      assert_eq!(model.id(), 1);
      assert_eq!(model.name(), "Caldari");
    }
  }
}
