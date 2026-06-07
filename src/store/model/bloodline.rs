use getset::{CopyGetters, Getters};
use sqlx::FromRow;

use crate::clients::esi::models::bloodlines::Bloodline;

#[derive(Clone, CopyGetters, Debug, Eq, FromRow, Getters, PartialEq)]
pub struct Model {
  #[getset(get_copy = "pub")]
  pub charisma: i32,
  #[getset(get_copy = "pub")]
  pub corporation_id: i64,
  #[getset(get = "pub")]
  pub description: String,
  #[getset(get_copy = "pub")]
  pub id: i64,
  #[getset(get_copy = "pub")]
  pub intelligence: i32,
  #[getset(get_copy = "pub")]
  pub memory: i32,
  #[getset(get = "pub")]
  pub name: String,
  #[getset(get_copy = "pub")]
  pub perception: i32,
  #[getset(get_copy = "pub")]
  pub race_id: i64,
  #[getset(get_copy = "pub")]
  pub ship_type_id: Option<i64>,
  #[getset(get_copy = "pub")]
  pub willpower: i32,
}

impl Model {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    id: i64,
    corporation_id: i64,
    race_id: i64,
    charisma: i32,
    description: impl Into<String>,
    intelligence: i32,
    memory: i32,
    name: impl Into<String>,
    perception: i32,
    willpower: i32,
  ) -> Self {
    Self {
      charisma,
      corporation_id,
      description: description.into(),
      id,
      intelligence,
      memory,
      name: name.into(),
      perception,
      race_id,
      ship_type_id: None,
      willpower,
    }
  }

  pub fn set_ship_type_id(&mut self, id: i64) {
    self.ship_type_id = Some(id);
  }
}

impl From<Bloodline> for Model {
  fn from(bloodline: Bloodline) -> Self {
    let mut model = Self::new(
      i64::from(bloodline.bloodline_id),
      bloodline.corporation_id,
      i64::from(bloodline.race_id),
      bloodline.charisma,
      bloodline.description,
      bloodline.intelligence,
      bloodline.memory,
      bloodline.name,
      bloodline.perception,
      bloodline.willpower,
    );

    if let Some(ship_type_id) = bloodline.ship_type_id {
      model.set_ship_type_id(i64::from(ship_type_id));
    }

    model
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod from {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_widens_ids_and_sets_ship_type() {
      let bloodline = Bloodline {
        bloodline_id: 1,
        charisma: 6,
        corporation_id: 1_000_006,
        description: "The Deteis.".to_owned(),
        intelligence: 7,
        memory: 7,
        name: "Deteis".to_owned(),
        perception: 5,
        race_id: 1,
        ship_type_id: Some(601),
        willpower: 5,
      };

      let model = Model::from(bloodline);

      assert_eq!(model.id(), 1);
      assert_eq!(model.race_id(), 1);
      assert_eq!(model.ship_type_id(), Some(601));
    }

    #[test]
    fn it_leaves_ship_type_none_when_the_bloodline_has_no_ship() {
      let bloodline = Bloodline {
        bloodline_id: 1,
        charisma: 6,
        corporation_id: 1_000_006,
        description: "The Deteis.".to_owned(),
        intelligence: 7,
        memory: 7,
        name: "Deteis".to_owned(),
        perception: 5,
        race_id: 1,
        ship_type_id: None,
        willpower: 5,
      };

      let model = Model::from(bloodline);

      assert_eq!(model.ship_type_id(), None);
    }
  }
}
